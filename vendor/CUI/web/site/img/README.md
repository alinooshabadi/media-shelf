# Images

`tetons.jpg`: Ansel Adams, *The Tetons and the Snake River* (1942), Grand Teton
National Park, Wyoming. Taken for the U.S. Department of the Interior and held
by the U.S. National Archives (record group 79). As a work of the U.S. federal
government it is in the public domain. Source:
https://commons.wikimedia.org/wiki/File:Adams_The_Tetons_and_the_Snake_River.jpg

The file here is cropped, converted to grayscale and reduced to 1000 px wide.
The overview page draws it as an ordered dither in the page's ink color
(`js/dither.js`).

`cui-mark.svg`: the CUI mark, a raster C of 47 rounded cells on a 9 by 9 grid.
It draws in `currentColor`; the edge cells use `fill-opacity` for the
anti-aliased steps, so it works on any background. `cui-mark-light.svg`
(ink `#1f2124`, for light backgrounds) and `cui-mark-dark.svg` (ink `#e9eaed`,
for dark backgrounds) are the same mark with a fixed ink, for places where
`currentColor` cannot reach (an `img` tag, a README). `favicon.svg` switches
between the two inks with the system color scheme. The top bar inlines the
mark so it follows the site's own theme toggle.
