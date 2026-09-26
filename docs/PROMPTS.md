# Making folder art in Grok or ChatGPT

You don't need an API key to make your own skins. Any chat app that can edit a picture
(Grok, ChatGPT) can paint a folder for you if you give it our template and the prompt below.
The prompt asks for a flat magenta background, which FolderSkin cuts away before adding the
folder to **Yours**.

## 1. Attach the template

Download [`prompts/folder-template.png`](prompts/folder-template.png) and attach it to your
message. It is a blank FolderSkin folder on a transparent background. The model repaints it,
so every result keeps the same shape: tab on the top left, a cream paper sheet between the
panels, and the front panel carrying the picture.

## 2. Paste the prompt

Fill in the two lines in capitals. Keep the rest as it is. Every sentence is there to stop a
specific mistake (a tilted folder, a drop shadow, pink bleeding into the art).

```text
Repaint the attached folder icon. Keep its exact shape: the same folder silhouette, the tab on
the top left, the thin cream paper sheet between the back and front panels, and the same size
and position in the frame. Straight-on front view, no perspective, no tilt.

Scene: DESCRIBE THE SCENE
Style: DESCRIBE THE STYLE

Paint the scene across the whole folder. The sky or background continues up into the back
panel and the tab. The main subject sits in the middle of the front panel, fully inside it.
Keep the paper sheet as a clean cream strip. Rich colour, strong light, fine texture, like a
collectible poster.

Paint everything outside the folder pure flat magenta #FF00FF: no shadow, no glow, no
gradient, no border, no other objects. Do not use magenta or hot pink inside the folder.
Square image.
```

Want lettering on it? Add one line before the last paragraph:
`Add the title "YOUR WORDS" in bold poster lettering on the front panel.` Keep it to one or
two words, because long text comes out garbled.

## 3. Bring it into FolderSkin

Save the image, then drop it on the FolderSkin window or use **Add your photo**. The magenta is
removed automatically and the folder appears under **Yours**, ready to apply.

If the edges show a pink fringe, the model drifted from pure magenta. Ask it to "repaint the
background as exactly #FF00FF" and try again.

## Styles that suit a folder

Describe one after `Style:`, or mix two. These are the thirty styles FolderSkin's own prompt box
offers when you type /. Under **No API key?**, the app fills in the exact words it uses for the
style you pick.

**Photo and 3D**

- **Studio photo**: a true-to-life photograph in soft studio light
- **Film photo**: a cinematic 35 mm photograph with natural light and grain
- **Product render**: a polished 3D render with studio light and real materials
- **Isometric 3D**: a tidy 3D model at the isometric angle
- **Tilt-shift miniature**: a real scene photographed like a tiny scale model

**Materials and craft**

- **Clay**: soft handmade modelling clay in warm light
- **Glass**: sculpted translucent glass with refraction and caustics
- **Neon**: glowing neon tubes against deep darkness
- **Enamel**: glossy cloisonné enamel with raised gold outlines
- **Embroidery**: dense satin-stitch embroidery in raised glossy thread
- **Papercut**: layered cut paper with real shadows between the layers
- **Stained glass**: jewel-toned leaded glass lit from behind

**Painting and drawing**

- **Oil painting**: rich classical oil paint with dramatic light
- **Watercolour**: loose, luminous transparent washes
- **Gouache**: opaque matte paint in flat, harmonious shapes
- **Pencil**: a detailed graphite drawing with a full tonal range
- **70s airbrush**: silky gradients, glossy chrome and star glints

**Print**

- **Woodblock print**: carved keylines, flat colour and wood grain
- **Linocut**: bold carved relief in black and one ink
- **Risograph**: grainy spot colour with slight misregistration
- **Travel poster**: mid-century flat colour with bold light
- **Pop art**: black outlines, primaries and halftone dots
- **Collage**: surreal cut paper and photographic fragments

**Digital and graphic**

- **Pixel art**: crisp 16-bit pixels and a limited palette
- **Blueprint**: white technical linework on deep blue
- **Low poly**: faceted 3D in flat-shaded triangles
- **Anime**: clean cel-shaded illustration
- **Art nouveau**: whiplash lines, gold outlines and jewel tones
- **Art deco**: bold symmetrical geometry in gold and black
- **Synthwave**: neon-lit 1980s chrome and haze

## Ideas to start from

Each line is a subject, written for the style in bold. Paste it after `Scene:` and describe that
style after `Style:`. These are the ideas the style buttons under a new chat fill in, two per
style. A word in quotes is meant as lettering on the folder.

- **Travel poster.** A tiny red seaplane landing on a turquoise lagoon at sunset, palm silhouettes and a low orange sun, with the word “ESCAPE”.
- **Travel poster.** A cable car climbing past snowy peaks toward a little alpine hotel, in flat blues and whites with one red accent.
- **Woodblock print.** A giant koi leaping from a moonlit river under a pale moon, in indigo and vermilion.
- **Woodblock print.** A fox in a straw hat crossing a lantern-lit bridge in the rain, with fine rain lines.
- **70s airbrush.** A chrome cassette tape floating over a desert highway at dusk, under a purple-to-tangerine sky.
- **70s airbrush.** A shiny roller skate orbiting a ringed planet, with chrome reflections and lens flares against a deep violet starfield.
- **Collage.** A vintage astronaut floating between paper clouds, holding a steaming coffee cup.
- **Collage.** A giant hand watering a tiny city skyline like a houseplant, under a mustard-yellow sky.
- **Art nouveau.** A woman whose hair turns into ocean waves, among lilies, in muted teal, cream and coral.
- **Art nouveau.** A peacock perched on a crescent moon among swirling vines, in jewel greens and blues.
- **Oil painting.** A cat in a velvet cloak holding a tiny laptop, lit by candlelight, in deep reds and golds.
- **Oil painting.** A whale drifting over a sleepy harbour at dawn, with soft clouds and warm morning light.
- **Film photo.** A lone red phone booth on a snowy mountain ridge at golden hour, with long shadows.
- **Film photo.** A vintage convertible parked outside a glowing roadside diner on a rainy night, with wet reflections.
- **Tilt-shift miniature.** A busy little post office built inside a wooden drawer, tiny workers sorting letters under warm lamps.
- **Tilt-shift miniature.** A tiny campsite on top of a giant open book, with a tent, a campfire and paper pine trees in soft evening light.
- **Risograph.** A vinyl record rising like the sun over desert dunes, in teal, yellow and orange.
- **Risograph.** A paper boat sailing through a city of stacked books, in blue and orange.
- **Clay.** A tiny lighthouse on a rocky island throwing a rainbow beam through puffy clouds, in pastel colours.
- **Clay.** A snail carrying a little house with glowing windows through a mossy forest, in soft light.

Or let the folder's name pick the scene: *"Scene: a witty poster about a folder called
'Taxes 2025'"*.

## Why magenta

Most chat apps return pictures without transparency. A flat #FF00FF background almost never
occurs in real artwork, so FolderSkin can find it, remove it and soften the edge cleanly. The
in-app assistant uses the same trick, with green instead of magenta for Google's models and for
pink or violet art, which it cuts out itself.

If a chat app hands back a picture that really is transparent around the folder, that works
too: FolderSkin uses the transparency as it is.
