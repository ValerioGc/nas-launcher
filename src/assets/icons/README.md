# Icons

Local SVGs on a 24×24 grid, rounded strokes and names joined with `_`.
The folders separate navigation, system, network and states. Each family has a colour, with
red/amber/green for error/warning/success. They contain no script and no external resource, and
their `<title>` is in English like the rest of the code.

`CommonIcon` uses the SVG as a background image, keeping its palette. The component handles the
three sizes (`small`, `medium`, `large`) and the semantics: `label` gives the accessible name;
without a label the icon is decorative. A button that is only an icon must have an accessible name.

`tinted` is the exception: it draws the icon in the current text colour, using the shape as a mask.
It is for a control that has to follow the theme, the close button of a tab on a surface whose
colour changes, where the fixed colour of the family is exactly the problem.

The logos of the applications live in the folders of the applications and are shown with `img`, not
as glyphs. Each application has its own pair of colours. Icons with no reference in the source have
been removed.
