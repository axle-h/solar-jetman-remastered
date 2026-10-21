//! The strings of the approach screen, the surface menu and the message screens
//! (`docs/spec/text.md` 3, 4.2-4.4, 4.6). Text is committed data.

/// The 14 planet names with their numbers (4.3); the 14th is shown only for level 13.
pub const PLANETS: [&str; 14] = [
    "1 PRELUDON",
    "2 MEXOMORF",
    "3 OMEBRU",
    "4 CORSO QWERO",
    "5 BOKKY",
    "6 LEMONTE",
    "7 CHORLTON",
    "8 SHISHKEBAB",
    "9 ZLAZ TORDUS",
    "10 SHAMMY GEN",
    "11 SHANKOO",
    "12 MIPLEZUR",
    "13 UROWND",
    "14 DRYN KUP LEEZ",
];

/// The five systems (4.2); the level index divided by three picks one.
pub const SYSTEMS: [&str; 5] = ["ZANDOR", "FROBOF", "MUGRIM", "SHOBAS", "XEXORM"];

/// The 17 cargo names of the analysis screen (4.4).
pub const CARGO: [&str; 17] = [
    "TENNIS BALLS",
    "HAMBURGERS",
    "JELLYBEANS",
    "EMPTY CONTAINERS",
    "ABSOLUTELY NOTHING",
    "TRIBBLES",
    "BANANAS",
    "SPEAKERS",
    "SOFT DRINKS",
    "GEOLOGICAL SAMPLES",
    "ROBOT KIT",
    "ENGLISH PHONE BOX",
    "COMPUTER GEAR",
    "EASTER ISLAND HEADS",
    "SOME GRASS",
    "MICROWAVE OVENS",
    "A PILE OF GEMS",
];

/// The analysis screen's digit table (4.4): its low-nibble index picks a character.
pub const ANALYSIS_DIGITS: &[u8; 16] = b"0143865498728380";

/// One line of a message: text at a column and row of the nametable (`{col,row}` of text.md 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub column: u8,
    pub row: u8,
    pub text: &'static str,
}

const fn line(column: u8, row: u8, text: &'static str) -> Line {
    Line { column, row, text }
}

/// A printer message: the lines written at once, then the lines typed out one character at a
/// time after `{TYPE}` (text.md 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub immediate: Vec<Line>,
    pub typed: Vec<Line>,
}

const ANALYSIS: [[Line; 3]; 17] = [
    [
        line(7, 10, "CARBON"),
        line(7, 12, "OXYGEN"),
        line(7, 14, "ZINC"),
    ],
    [
        line(7, 10, "LEAD"),
        line(7, 12, "SILICON"),
        line(7, 14, "TARROGON"),
    ],
    [
        line(7, 10, "OXYGEN"),
        line(7, 12, "HYDROGEN"),
        line(7, 14, "IRON"),
    ],
    [
        line(7, 10, "PINEAPPLE"),
        line(7, 12, "WOOD"),
        line(7, 14, "MOUSE"),
    ],
    [
        line(7, 10, "CALIFORNIUM"),
        line(7, 12, "GOLD"),
        line(7, 14, "NICKLE"),
    ],
    [
        line(7, 10, "ALUMINIUM"),
        line(7, 12, "LITHIUM"),
        line(7, 14, "KRYPTON"),
    ],
    [
        line(7, 10, "XENON"),
        line(7, 12, "ZIPPON"),
        line(7, 14, "FLUON"),
    ],
    [
        line(7, 10, "UNOBTANIUM"),
        line(7, 12, "LINOLIUM"),
        line(7, 14, "LONGONE"),
    ],
    [
        line(7, 10, "WATER"),
        line(7, 12, "SUGAR"),
        line(7, 14, "COLOURING"),
    ],
    [
        line(7, 10, "STONE"),
        line(7, 12, "PAPER"),
        line(7, 14, "SCISSORS"),
    ],
    [
        line(7, 10, "PRAWNS"),
        line(7, 12, "COCKLES"),
        line(7, 14, "MUSSLES"),
    ],
    [
        line(7, 10, "CHEESE"),
        line(7, 12, "STODGE"),
        line(7, 14, "FRENCH FRIES"),
    ],
    [
        line(7, 10, "RUBBER"),
        line(7, 12, "MOXXY"),
        line(7, 14, "VOOFORS"),
    ],
    [
        line(7, 10, "DETERMINON"),
        line(7, 12, "PHENOMENON"),
        line(7, 14, "EXISTENON"),
    ],
    [
        line(7, 10, "HEALTHY"),
        line(7, 12, "BONES"),
        line(7, 14, "TEETH"),
    ],
    [
        line(7, 10, "NEW"),
        line(7, 12, "BIOLOGICAL"),
        line(7, 14, "OMO"),
    ],
    [
        line(7, 10, "PLASTIC"),
        line(7, 12, "GLASS"),
        line(7, 14, "PEBBLES"),
    ],
];

/// The analysis screen's frame text, message 27.
const ANALYSIS_FRAME: [Line; 2] = [line(13, 2, "ANALYSIS"), line(10, 18, "VALUE")];

/// The message with printer id `id` (text.md 3), or `None` for an id that has no text here.
pub fn message(id: u8) -> Option<Message> {
    let message = |immediate: &[Line], typed: &[Line]| {
        Some(Message {
            immediate: immediate.to_vec(),
            typed: typed.to_vec(),
        })
    };
    match id {
        6 | 7 => message(
            &[],
            &[
                line(7, 15, "YOUR PROBES ARE NOW"),
                line(6, 16, "EQUIPPED WITH SHIELDS"),
                line(5, 17, "-PRESS DOWN TO ACTIVATE"),
            ],
        ),
        8 => message(
            &[],
            &[
                line(5, 15, "YOU HAVE EQUIPPED YOUR"),
                line(6, 16, "PROBES WITH BOOSTERS"),
                line(4, 17, "-SELECT AND B TO THRUST"),
            ],
        ),
        9 => message(
            &[],
            &[
                line(5, 16, "YOU HAVE EQUIPPED YOUR"),
                line(5, 17, "PROBES WITH NAVIGATORS"),
            ],
        ),
        21 => message(
            &[],
            &[
                line(7, 14, "YOU NEED MORE FUEL"),
                line(6, 16, "TO LEAVE THIS PLANET"),
            ],
        ),
        22 => message(
            &[],
            &[
                line(7, 12, "YOU HAVE COLLECTED"),
                line(10, 13, "ENOUGH  FUEL"),
                line(5, 15, "NOW FIND THE SHIP PART"),
                line(9, 17, "AND BLAST OFF"),
            ],
        ),
        23 => message(
            &[
                line(5, 6, "CONGRATULATIONS JETMAN"),
                line(7, 10, "YOU HAVE COMPLETED"),
                line(10, 12, "YOUR MISSION"),
                line(8, 16, "TIME TO PUT YOUR"),
                line(7, 18, "FEET UP  AND RELAX"),
                line(5, 20, "IN THE COMFORT OF YOUR"),
                line(5, 22, "LUXURY GOLDEN WARPSHIP"),
            ],
            &[],
        ),
        25 => message(
            &[line(8, 8, "STORING  CRYSTAL"), line(11, 13, "VALUE 500")],
            &[
                line(4, 16, "MORE CRYSTALS NEEDED FOR"),
                line(10, 17, "A BONUS LIFE"),
            ],
        ),
        27 => message(&ANALYSIS_FRAME, &[]),
        28..=44 => message(&ANALYSIS[usize::from(id - 28)], &[]),
        45 => message(
            &[
                line(9, 11, "FANCY ALIEN ITEMS"),
                line(13, 14, "VERY RARE"),
                line(12, 17, "VALUE  1000"),
            ],
            &[],
        ),
        46 => message(
            &[
                line(11, 8, "RADIOACTIVES"),
                line(12, 11, "URANIUM 235"),
                line(12, 13, "PLEBIUM 232"),
                line(12, 15, "QUORUM  220"),
                line(13, 17, "VALUE 750"),
            ],
            &[],
        ),
        47 => message(
            &[],
            &[
                line(5, 15, "YOU HAVE FOUND A PIECE"),
                line(5, 17, "OF THE GOLDEN WARPSHIP"),
            ],
        ),
        48 => message(
            &[],
            &[
                line(7, 14, "THE GOLDEN WARPSHIP"),
                line(9, 15, "IS NOW COMPLETE"),
                line(9, 17, "DESTINY SUMMONS"),
            ],
        ),
        50 => message(
            &[],
            &[
                line(4, 8, "YOU HAVE ENOUGH CRYSTALS"),
                line(12, 13, "YOU GAIN"),
                line(10, 15, "A BONUS LIFE"),
                line(8, 17, "AND 2000 CREDITS"),
            ],
        ),
        _ => None,
    }
}

/// The planet menu's row records (4.6): 11 tiles each, `1`-`6` are frame pieces.
pub const MENU_RECORDS: [&str; 10] = [
    "15555555552",
    "6         6",
    "35555555554",
    "",
    "6 EXPLORE 6",
    "6  EQUIP  6",
    "6   POD   6",
    "6  BLAST  6",
    "6   OFF   6",
    "6   BUY   6",
];

/// The record numbers of each layout, top to bottom (4.6).
pub const MENU_LAYOUTS: [&[u8]; 4] = [
    &[0, 1, 4, 1, 5, 6, 1, 2, 3],
    &[0, 1, 4, 1, 5, 6, 1, 7, 8, 1, 2, 3],
    &[0, 1, 4, 1, 2, 3],
    &[0, 1, 4, 1, 7, 8, 1, 2, 3],
];

/// How many options each layout offers.
pub const MENU_OPTIONS: [u8; 4] = [2, 3, 1, 2];

/// Text of a signed number as the approach screen shows it (4.3): a hyphen for a negative one,
/// no leading zeros.
pub fn signed_number(value: i32) -> String {
    format!("{value}")
}
