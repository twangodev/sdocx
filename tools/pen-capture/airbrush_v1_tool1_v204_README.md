# AirBrush V1 tool 1 additional controls

Three synthetic Model/Drawing captures extend the existing tool-1 diagnostic with a curved path (size 3.144), a turning path (6.5), and a vertical path (12), using two opaque colors and one translucent color. The retained receipt includes native source arrays, vectors and complete compressed frames. It is host Mesa evidence, not saved-note or Android-device parity. The capture script checks the same local runner and prerequisite hashes as the earlier tool-1 control; those local dependencies are not supplied in a clean checkout.

The historical isolated candidate added tool 1 to the existing opt-in V1 profile. For these inputs, its sampled vector bytes match native in all three cases. At 256×256, public resvg RGBA absolute errors against native are 12, 0, and 4 respectively; generic errors are 5431, 13310, and 17423. Historical direct PDF comparisons were produced by PyMuPDF 1.28.2 and PDFium 150.0.7869.0 at 96 dpi. Both readers improve over generic for all three inputs. The earlier size-2.25 reverse control still regresses in MuPDF, so the candidate remains unintegrated; these cases do not establish a reliable size boundary.

To reproduce the native capture, run `.venv/bin/python tools/pen-capture/airbrush_v1_tool1_v204_capture.py /tmp/airbrush-v204-new`. The destination must not exist and its parent must exist. Before creating it, the script checks the local runner and the hash-pinned `airbrush_v1_curved_prerequisites.json` manifest from the earlier tool-1 receipt, then checks the resulting capture hashes. Those local dependencies are not included in a clean checkout.

The historical candidate and reader comparisons used synthetic host inputs.
The experimental replay is retired; the native capture and receipt remain.
Additional PDF image-transport and alpha-coverage experiments did not isolate
the stage responsible for reader differences or establish a production fix.
