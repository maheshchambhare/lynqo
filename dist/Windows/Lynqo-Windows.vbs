Set WshShell = CreateObject("WScript.Shell")
' Run lynqo-server.exe in completely hidden window mode (0 = Hide window)
WshShell.Run chr(34) & WshShell.CurrentDirectory & "\lynqo-server.exe" & chr(34), 0, False
WScript.Sleep 1200
' Open web interface portal in default browser
WshShell.Run "http://localhost:7432"
