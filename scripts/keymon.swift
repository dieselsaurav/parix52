// Passive monitor: modifier flags + G/B key state from the HID system state.
import Foundation
import CoreGraphics
let secs = Double(CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "60") ?? 60
let end = Date().addingTimeInterval(secs)
let fmt = DateFormatter(); fmt.dateFormat = "HH:mm:ss.SSS"
var last = ""
while Date() < end {
    let f = CGEventSource.flagsState(.hidSystemState)
    var mods = ""
    if f.contains(.maskControl) { mods += "⌃" }
    if f.contains(.maskAlternate) { mods += "⌥" }
    if f.contains(.maskShift) { mods += "⇧" }
    if f.contains(.maskCommand) { mods += "⌘" }
    if mods.isEmpty { mods = "-" }
    let g = CGEventSource.keyState(.hidSystemState, key: 5) ? 1 : 0
    let b = CGEventSource.keyState(.hidSystemState, key: 11) ? 1 : 0
    let cur = "mods=\(mods) G=\(g) B=\(b)"
    if cur != last { print("\(fmt.string(from: Date()))  \(cur)"); fflush(stdout); last = cur }
    usleep(5000)
}
print("done")
