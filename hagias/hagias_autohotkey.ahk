#Requires AutoHotkey v2.0
#SingleInstance Force

; Define the function once
ApplyLayout(num) {
    Run(A_ScriptDir "\hagias.exe layout apply " num)
}

; Call the function for each hotkey
#^1::ApplyLayout(1)
#^2::ApplyLayout(2)
#^3::ApplyLayout(3)
#^4::ApplyLayout(4)
#^5::ApplyLayout(5)
#^6::ApplyLayout(6)
#^7::ApplyLayout(7)
#^8::ApplyLayout(8)
#^9::ApplyLayout(9)
