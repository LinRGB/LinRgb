# LinRGB 1.0.0-beta.3 — tray menu fix

Right-click the LinRGB tray icon to open the app, apply a saved profile, relaunch or quit. The native tray menu was confirmed working by the maintainer on NixOS. This is a desktop integration update to beta 2, not a change to hardware support or saved profile formats.

Quit the old instance with **Ctrl+Q**, install the new package, then launch LinRGB normally. `linrgb --version` should print `LinRGB 1.0.0-beta.3`. The menu requires a desktop with StatusNotifier/DBusMenu support.
