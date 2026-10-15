# Pictogram van MathPrime

Een afgeronde blauwe tegel met rasterpuntjes (zoals het werkblad), een integraalteken en `:=`.
Eigen ontwerp, getekend met System.Drawing (Windows PowerShell).

| bestand | gebruik |
|---|---|
| `mathprime.ico` | 16–256 px, voor Windows (Verkenner, snelkoppelingen) |
| `mathprime.png` | 256 px |
| `mathprime-128.rgba` | 128×128 RGBA, het vensterpictogram (`main.rs`) |
| `mathprime.rc` / `mathprime.res` | resource met het pictogram, gelinkt in het exe-bestand (`build.rs`) |

Opnieuw maken van de `.res` na een nieuw `.ico` (Windows SDK):

```
rc.exe /nologo /fo mathprime.res mathprime.rc
```
