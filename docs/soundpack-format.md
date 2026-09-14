# IDzVibes Soundpack Format (`.idzpack`)

An `.idzpack` is a directory or zip archive structured as follows:

```
my_soundpack/
├── metadata.json
├── preview.mp3       (optional)
├── icon.png          (optional)
└── sounds/
    ├── generic/
    │   ├── 01.wav
    │   ├── 02.wav
    │   └── up.wav
    ├── space/
    │   ├── 01.wav
    │   └── up.wav
    ├── enter/
    │   ├── 01.wav
    │   └── up.wav
    └── mouse/
        ├── click_down.wav
        └── click_up.wav
```

## `metadata.json` Schema

```json
{
  "id": "creamy_thock",
  "name": "Creamy Thock",
  "author": "IDzVibes Studio",
  "version": "1.0.0",
  "description": "Deep mechanical keyboard sound with tuned acoustics",
  "license": "MIT",
  "tags": ["mechanical", "thocky", "creamy"],
  "has_release_sounds": true
}
```
