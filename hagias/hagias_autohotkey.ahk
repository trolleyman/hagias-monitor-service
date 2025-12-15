#Requires AutoHotkey v2.0
#SingleInstance Force

; Define the function once
ApplyLayout(num) {
    Run(A_ScriptDir "\hagias.exe layout apply " num)
}

; Call the function for each hotkey
#^1::ApplyLayout(0)
#^2::ApplyLayout(1)
#^3::ApplyLayout(2)
#^4::ApplyLayout(3)
#^5::ApplyLayout(4)
#^6::ApplyLayout(5)
#^7::ApplyLayout(6)
#^8::ApplyLayout(7)
#^9::ApplyLayout(8)
#^0::ApplyLayout(9)
