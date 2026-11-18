# LinRgb design preview

Open `index.html` in a browser. The **Appearance** menu offers LinRgb Light, LinRgb Dark, and a mode that follows the system. The preview does not change hardware lighting or the native GTK application.

## Create a theme

Copy `theme.example.json`, choose `light` or `dark`, and edit the nine `#RRGGBB` values. Choose **Import theme…** in the Appearance menu to load the file. Check text and button contrast in the preview. The latest imported theme and your selection are saved if the browser permits local storage for this page.

## Generate a theme with Matugen

Add this block to your Matugen configuration, replacing `/absolute/path/to/LinRgb` with your project path:

```toml
[templates.linrgb]
input_path = "/absolute/path/to/LinRgb/design/theme.matugen.json.tmpl"
output_path = "~/.config/matugen/linrgb-theme.json"
```

Generate the palette in dark mode, then import `~/.config/matugen/linrgb-theme.json` from the Appearance menu:

```bash
matugen image /path/to/wallpaper.jpg --mode dark
```

To generate a light palette, copy the template, change its `mode` to `light`, point `input_path` at the copy, and use `--mode light`. Re-import the generated JSON after changing wallpapers; the browser does not watch the file.
