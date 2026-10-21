//! The clean-room check: no decompiler identifiers or 6502 assembly anywhere in the repository
//! (docs/CLEAN_ROOM.md). It also keeps the platform-free modules free of I/O and platform code.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// What the scan looks at, relative to the repository root.
const ROOTS: &[&str] = &[
    "src",
    "tests",
    "build.rs",
    "data",
    "docs",
    "tools",
    "README.md",
    "CLAUDE.md",
];
const SKIPPED_DIRS: &[&str] = &["target", ".git"];
/// The modules that must build anywhere, the web included: no file, window or audio I/O.
const PLATFORM_FREE: &[&str] = &["src/game", "src/rom", "src/apu"];

pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub kind: &'static str,
    pub text: String,
}

pub struct Scanner {
    rules: Vec<(&'static str, Regex)>,
}

impl Scanner {
    pub fn new() -> Self {
        // Names Ghidra generates for functions, data, labels, variables and parameters.
        let decompiler = Regex::new(concat!(
            r"\b(?:FUN|DAT|LAB|PTR|SUB|switchD|caseD|thunk_FUN|joined_r0x|code_r0x)_[0-9A-Fa-f_]+\b",
            r"|\b(?:[a-z]{1,3}Var\d+|param_\d+|local_[0-9a-f]+|[a-z]{1,3}Stack_[0-9a-f]+",
            r"|in_stack_[0-9a-f]+|extraout_\w+|unaff_\w+|in_[A-Z]{1,2})\b",
        ))
        .expect("decompiler pattern");
        // A 6502 instruction with an operand, as in a disassembly listing: anywhere with an
        // immediate operand, otherwise only where the mnemonic opens the line (after an
        // optional comment marker, backtick, address or label), so that prose such as
        // "the byte AND $F8" passes.
        let mnemonic = concat!(
            "(?:LDA|LDX|LDY|STA|STX|STY|ADC|SBC|AND|ORA|EOR|CMP|CPX|CPY|INC|DEC|ASL|LSR|ROL|ROR",
            "|BIT|JMP|JSR|BNE|BEQ|BPL|BMI|BCC|BCS|BVC|BVS)",
        );
        let assembly = Regex::new(&format!(
            r"\b{mnemonic}\s+#|(?m)^\s*(?:(?://+|;|\*|-|>)\s*)?`?(?:\$?[0-9A-Fa-f]{{4}}:?\s+)?(?:\w+:\s*)?{mnemonic}\s+[$(]"
        ))
        .expect("assembly pattern");
        Scanner {
            rules: vec![
                ("decompiler identifier", decompiler),
                ("6502 assembly", assembly),
            ],
        }
    }

    pub fn scan_text(&self, text: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (index, line) in text.lines().enumerate() {
            for (kind, pattern) in &self.rules {
                for found in pattern.find_iter(line) {
                    findings.push(Finding {
                        line: index + 1,
                        column: line[..found.start()].chars().count() + 1,
                        kind,
                        text: found.as_str().to_owned(),
                    });
                }
            }
        }
        findings
    }
}

fn text_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let skipped = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| SKIPPED_DIRS.contains(&name));
        if skipped {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        children.sort();
        for child in children {
            text_files(&child, out);
        }
    } else if path.is_file() {
        out.push(path.to_owned());
    }
}

/// Scans the repository at `root`; returns the number of findings, after printing each one.
pub fn run(root: &Path) -> usize {
    let scanner = Scanner::new();
    let mut files = Vec::new();
    for entry in ROOTS {
        text_files(&root.join(entry), &mut files);
    }
    let mut scanned = 0;
    let mut problems = 0;
    for path in &files {
        let Ok(bytes) = fs::read(path) else { continue };
        if bytes.iter().take(8192).any(|&b| b == 0) {
            continue; // binary
        }
        scanned += 1;
        let text = String::from_utf8_lossy(&bytes);
        for finding in scanner.scan_text(&text) {
            let shown = path.strip_prefix(root).unwrap_or(path).display();
            eprintln!(
                "{shown}:{}:{}: {} \"{}\"",
                finding.line, finding.column, finding.kind, finding.text
            );
            problems += 1;
        }
    }
    problems += platform_free(root);
    if problems == 0 {
        println!("Clean-room check: {scanned} files, nothing found.");
    } else {
        eprintln!("\n{problems} finding(s). See docs/CLEAN_ROOM.md.");
    }
    problems
}

/// Flags I/O, platform crates and the tools in the platform-free modules; returns the count.
fn platform_free(root: &Path) -> usize {
    let pattern =
        Regex::new(r"\b(?:sdl3|directories|crate::tools|std::(?:fs|net|process|thread|env))\b")
            .expect("platform pattern");
    let mut files = Vec::new();
    for entry in PLATFORM_FREE {
        text_files(&root.join(entry), &mut files);
    }
    let mut problems = 0;
    for path in files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
    {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            for found in pattern.find_iter(line) {
                let shown = path.strip_prefix(root).unwrap_or(path).display();
                eprintln!(
                    "{shown}:{}: platform code in a platform-free module \"{}\"",
                    index + 1,
                    found.as_str()
                );
                problems += 1;
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::Scanner;

    // Forbidden tokens are assembled at runtime so that this file passes the check itself.
    fn joined(parts: &[&str]) -> String {
        parts.concat()
    }

    #[test]
    fn flags_decompiler_identifiers() {
        let scanner = Scanner::new();
        for token in [
            joined(&["FUN", "_0c8a40"]),
            joined(&["DAT", "_0042"]),
            joined(&["LAB", "_8c40"]),
            joined(&["u", "Var1"]),
            joined(&["b", "Var12"]),
            joined(&["param", "_1"]),
            joined(&["local", "_1c"]),
            joined(&["in", "_stack_00000004"]),
            joined(&["extraout", "_A"]),
            joined(&["unaff", "_X"]),
        ] {
            let text = format!("let speed = {token};");
            assert_eq!(scanner.scan_text(&text).len(), 1, "{token}");
        }
    }

    #[test]
    fn flags_assembly() {
        let scanner = Scanner::new();
        for line in [
            joined(&["  LD", "A #$03"]),
            joined(&["ST", "A $0200,X"]),
            joined(&["JS", "R $C000"]),
            joined(&["LD", "A ($10),Y"]),
            joined(&["// AN", "D $F8"]),
            joined(&["C040: ST", "A $2006"]),
            joined(&["`BI", "T $2002`"]),
            joined(&["the value AN", "D #$F8"]),
        ] {
            assert_eq!(scanner.scan_text(&line).len(), 1, "{line}");
        }
    }

    #[test]
    fn passes_ordinary_text() {
        let scanner = Scanner::new();
        let text = "The ship's thrust AND gravity (in pixels) are read at RAM $0042.\n\
                    | $6F | the byte AND $F8, then ORA $03 in effect |\n\
                    let fun_count = local_speed + param; // DATA_SIZE, FUN_, uVar";
        assert!(scanner.scan_text(text).is_empty());
    }
}
