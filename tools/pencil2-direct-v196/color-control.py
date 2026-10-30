import pymupdf as fitz
import pypdfium2 as pdfium

if not __debug__:
    raise RuntimeError('This control requires assertions; do not run Python with -O')
if fitz.VersionBind != '1.28.2' or str(pdfium.PDFIUM_INFO) != '156.0.8076.0':
    raise RuntimeError(f'Expected MuPDF 1.28.2 and PDFium 156.0.8076.0; got {fitz.VersionBind} and {pdfium.PDFIUM_INFO}')


def render(masked, color, backdrop):
    doc = fitz.open()
    page = doc.new_page(width=2, height=2)
    alpha = doc.get_new_xref()
    doc.update_object(alpha, '<</Type/ExtGState/ca .2>>')
    form = doc.get_new_xref()
    doc.update_object(form, f'<</Type/XObject/Subtype/Form/BBox[0 0 2 2]/Group<</S/Transparency/CS/DeviceRGB/I true>>/Resources<</ExtGState<</A {alpha} 0 R>>>>>>')
    doc.update_stream(form, b'0 0 0 rg 0 0 2 2 re f 1 1 1 rg /A gs 0 1 1 1 re f 0 1 1 1 re f 0 1 1 1 re f')
    state = doc.get_new_xref()
    doc.update_object(state, f'<</Type/ExtGState/SMask<</S/Luminosity/G {form} 0 R>>>>' if masked else '<</Type/ExtGState/ca .2>>')
    doc.xref_set_key(page.xref, 'Resources', f'<</ExtGState<</G {state} 0 R>>>>')
    stream = doc.get_new_xref()
    doc.update_object(stream, '<<>>')
    prefix = {
        'transparent': b'',
        'blue': b'0 0 1 rg 0 0 2 2 re f ',
        'white': b'1 1 1 rg 0 0 2 2 re f ',
    }[backdrop]
    paint = f'{color[0]} {color[1]} {color[2]} rg 0 1 1 1 re f '.encode()
    doc.update_stream(stream, prefix + b'/G gs ' + paint * (1 if masked else 3))
    doc.xref_set_key(page.xref, 'Contents', f'{stream} 0 R')
    data = doc.tobytes()
    with fitz.open(stream=data, filetype='pdf') as other:
        mu = tuple(other[0].get_pixmap(alpha=True).samples[:4])
    with pdfium.PdfDocument(data) as other:
        pi = other[0].render(fill_color=(0, 0, 0, 0), rev_byteorder=True).to_pil().convert('RGBA').getpixel((0, 0))
    print('mask' if masked else 'direct', color, backdrop, 'MuPDF', mu, 'PDFium', pi)


for color in [(0, 0, 0), (1, 0, 0), (0, 1, 0)]:
    for backdrop in ('transparent', 'blue', 'white'):
        for masked in (False, True):
            render(masked, color, backdrop)


def mixed_control(backdrop, colors):
    doc = fitz.open()
    page = doc.new_page(width=2, height=2)
    alpha = doc.get_new_xref()
    doc.update_object(alpha, '<</Type/ExtGState/ca .2>>')
    form = doc.get_new_xref()
    doc.update_object(form, f'<</Type/XObject/Subtype/Form/BBox[0 0 2 2]/Group<</S/Transparency/CS/DeviceRGB/I true>>/Resources<</ExtGState<</A {alpha} 0 R>>>>>>')
    doc.update_stream(form, b'0 0 0 rg 0 0 2 2 re f 1 1 1 rg /A gs 0 1 1 1 re f 0 1 1 1 re f 0 1 1 1 re f')
    state = doc.get_new_xref()
    doc.update_object(state, f'<</Type/ExtGState/SMask<</S/Luminosity/G {form} 0 R>>>>')
    doc.xref_set_key(page.xref, 'Resources', f'<</ExtGState<</G {state} 0 R>>>>')
    stream = doc.get_new_xref()
    doc.update_object(stream, '<<>>')
    prefix = {
        'transparent': '',
        'blue': '0 0 1 rg 0 0 2 2 re f ',
        'white': '1 1 1 rg 0 0 2 2 re f ',
    }[backdrop]
    paints = ''.join(f'{r} {g} {b} rg 0 1 1 1 re f ' for r, g, b in colors)
    doc.update_stream(stream, (prefix + '/G gs ' + paints).encode())
    doc.xref_set_key(page.xref, 'Contents', f'{stream} 0 R')
    data = doc.tobytes()
    with fitz.open(stream=data, filetype='pdf') as other:
        mu = tuple(other[0].get_pixmap(alpha=True).samples[:4])
    with pdfium.PdfDocument(data) as other:
        pi = other[0].render(fill_color=(0, 0, 0, 0), rev_byteorder=True).to_pil().convert('RGBA').getpixel((0, 0))
    pi = tuple(round(c * pi[3] / 255) for c in pi[:3]) + (pi[3],)
    base = {'transparent': (0, 0, 0, 0), 'blue': (0, 0, 1, 1), 'white': (1, 1, 1, 1)}[backdrop]
    opacity = 1 - .8 ** 3
    expected = list(base)
    for color in colors:
        expected = [color[i] * opacity + expected[i] * (1 - opacity) for i in range(3)] + [opacity + expected[3] * (1 - opacity)]
    expected = tuple(round(value * 255) for value in expected)
    for name, actual in [('MuPDF', mu), ('PDFium', pi)]:
        assert max(abs(a - b) for a, b in zip(actual, expected)) <= 5, (backdrop, colors, name, actual, expected)
        print('mixed mask', backdrop, colors, name, 'premultiplied', actual, 'ideal', expected)
    return mu, pi


red, green = (1, 0, 0), (0, 1, 0)
for backdrop in ('transparent', 'blue', 'white'):
    forward = mixed_control(backdrop, (red, green))
    reverse = mixed_control(backdrop, (green, red))
    for first, second in zip(forward, reverse):
        assert first[1] > first[0] and second[0] > second[1], (backdrop, first, second)
