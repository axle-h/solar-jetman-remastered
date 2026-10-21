-- Smoke test for tools/mesen/run.sh: runs 300 frames and exits with code 0.
local frames = 0
emu.addEventCallback(function()
  frames = frames + 1
  if frames == 300 then
    print("ran " .. frames .. " frames, screen buffer " .. #emu.getScreenBuffer() .. " pixels")
    emu.stop(0)
  end
end, emu.eventType.endFrame)
