import pymupdf as fitz
import pypdfium2 as pdfium


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
