Optional: drop a .ttf/.otf font here and the "6-SEVEN" logo will use it — no
system font install needed (great for a headless Raspberry Pi).

The logo prefers (in order): Arial Black, Arial, Liberation Sans, DejaVu Sans,
FreeSans, then ANY available font. So:

- On a normal Raspberry Pi OS it already works (uses DejaVu Sans Bold).
- For the bold "Impact" look, download a heavy display font (e.g. Anton, OFL
  licensed) and drop Anton-Regular.ttf in this folder, then edit the logo's
  font-family in src/receipt.rs (logo_svg) to start with 'Anton'.

If the logo prints blank on a minimal/headless Pi, it has NO fonts at all —
install one:  sudo apt-get install -y fonts-dejavu-core
