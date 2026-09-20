from pathlib import Path
import pymupdf as fitz
root=Path(__file__).resolve().parents[1]/'crates/core/tests/fixtures'
d=fitz.open(); p=d.new_page(width=600,height=800); p.insert_text((70,100),'Original content',fontsize=20); p.add_text_annot((100,150),'Existing annotation'); p.set_cropbox(fitz.Rect(40,60,540,760)); p.set_rotation(90);d.save(root/'cropped-rotated.pdf')
d.save(root/'password.pdf',encryption=fitz.PDF_ENCRYPT_AES_256,owner_pw='owner-test',user_pw='open-test',permissions=fitz.PDF_PERM_PRINT|fitz.PDF_PERM_MODIFY|fitz.PDF_PERM_COPY|fitz.PDF_PERM_ANNOTATE|fitz.PDF_PERM_ASSEMBLE)
