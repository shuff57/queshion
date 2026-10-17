#!/usr/bin/env python3
"""Rebuilds the binary import fixtures (zip/xlsx) in this folder, deterministically.

    python3 -I tests/import_fixtures/make_binary_fixtures.py

Writes canvas_qti_export.zip, qti21_package.zip, qti21_choice_multiple.xml, moodle_image.xml,
qti3_item.xml, questions.xlsx, kahoot.xlsx and vocab.xlsx. Text fixtures are
hand-written and not touched. Standard library only.
"""
import struct
import zipfile
import zlib
from pathlib import Path
from xml.sax.saxutils import escape

HERE = Path(__file__).resolve().parent
STAMP = (2024, 1, 1, 0, 0, 0)


def png(w=2, h=2, rgb=(200, 40, 40)):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


def write_zip(name, files):
    with zipfile.ZipFile(HERE / name, "w", zipfile.ZIP_DEFLATED) as z:
        for path, data in files:
            info = zipfile.ZipInfo(path, STAMP)
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, data if isinstance(data, bytes) else data.encode("utf-8"))


# ---------- Canvas QTI export (QTI 1.2) ----------

def canvas_zip():
    quiz = (HERE / "qti12_canvas_quiz.xml").read_text()
    manifest = """<?xml version="1.0" encoding="UTF-8"?>
<manifest identifier="gmanifest" xmlns="http://www.imsglobal.org/xsd/imsccv1p1/imscp_v1p1">
  <metadata><schema>IMS Content</schema><schemaversion>1.1.3</schemaversion>
    <imsmd:lom xmlns:imsmd="http://ltsc.ieee.org/xsd/imsccv1p1/LOM/resource"><imsmd:general><imsmd:title><imsmd:string>QTI Export for Atoms quiz</imsmd:string></imsmd:title></imsmd:general></imsmd:lom>
  </metadata>
  <organizations/>
  <resources>
    <resource identifier="gquiz1" type="imsqti_xmlv1p2"><file href="gquiz1/gquiz1.xml"/><dependency identifierref="gquiz1meta"/></resource>
    <resource identifier="gquiz1meta" type="associatedcontent/imscc_xmlv1p1/learning-application-resource" href="gquiz1/assessment_meta.xml"><file href="gquiz1/assessment_meta.xml"/></resource>
    <resource identifier="gfiles" type="webcontent" href="Quiz Files/atom.png"><file href="Quiz Files/atom.png"/></resource>
  </resources>
</manifest>
"""
    meta = """<?xml version="1.0" encoding="UTF-8"?>
<quiz identifier="gquiz1" xmlns="http://canvas.instructure.com/xsd/cccv1p0">
  <title>Atoms quiz</title><description>&lt;p&gt;Chapter 2&lt;/p&gt;</description><quiz_type>assignment</quiz_type><points_possible>13.0</points_possible>
</quiz>
"""
    write_zip("canvas_qti_export.zip", [
        ("imsmanifest.xml", manifest),
        ("gquiz1/gquiz1.xml", quiz),
        ("gquiz1/assessment_meta.xml", meta),
        # Common Cartridge exports carry each quiz a second time; the importer dedupes items.
        ("non_cc_assessments/gquiz1.xml.qti", quiz),
        ("Quiz Files/atom.png", png()),
    ])


# ---------- QTI 2.1 content package ----------

NS = 'xmlns="http://www.imsglobal.org/xsd/imsqti_v2p1"'


def item21(ident, title, decls, body, extra=""):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<assessmentItem {NS} identifier="{ident}" title="{escape(title)}" adaptive="false" timeDependent="false">
{decls}
  <outcomeDeclaration identifier="SCORE" cardinality="single" baseType="float"/>
  <outcomeDeclaration identifier="MAXSCORE" cardinality="single" baseType="float"><defaultValue><value>{extra or 1}</value></defaultValue></outcomeDeclaration>
  <itemBody>
{body}
  </itemBody>
  <responseProcessing template="http://www.imsglobal.org/question/qti_v2p1/rptemplates/match_correct"/>
</assessmentItem>
"""


def decl(ident, card, base, values=(), mapping=()):
    v = "".join(f"<value>{escape(x)}</value>" for x in values)
    c = f"<correctResponse>{v}</correctResponse>" if values else ""
    m = ""
    if mapping:
        m = '<mapping defaultValue="0">' + "".join(f'<mapEntry mapKey="{escape(k)}" mappedValue="{s}"/>' for k, s in mapping) + "</mapping>"
    return f'  <responseDeclaration identifier="{ident}" cardinality="{card}" baseType="{base}">{c}{m}</responseDeclaration>'


ITEMS21 = {
    "items/q_choice.xml": item21("choice1", "Lab safety", decl("RESPONSE", "single", "identifier", ["B"]), """
    <p>Which piece of glassware is shown?</p>
    <p><img src="../images/flask.png" alt="glassware"/></p>
    <choiceInteraction responseIdentifier="RESPONSE" shuffle="true" maxChoices="1">
      <simpleChoice identifier="A">Beaker</simpleChoice>
      <simpleChoice identifier="B">Erlenmeyer flask</simpleChoice>
      <simpleChoice identifier="C" fixed="true">None of these</simpleChoice>
    </choiceInteraction>""", extra="2"),
    "items/q_match.xml": item21("match1", "Units", decl("RESPONSE", "multiple", "directedPair", ["L1 R1", "L2 R2"]), """
    <matchInteraction responseIdentifier="RESPONSE" shuffle="true" maxAssociations="2">
      <prompt>Match each quantity to its SI unit.</prompt>
      <simpleMatchSet>
        <simpleAssociableChoice identifier="L1" matchMax="1">Mass</simpleAssociableChoice>
        <simpleAssociableChoice identifier="L2" matchMax="1">Length</simpleAssociableChoice>
      </simpleMatchSet>
      <simpleMatchSet>
        <simpleAssociableChoice identifier="R1" matchMax="1">kilogram</simpleAssociableChoice>
        <simpleAssociableChoice identifier="R2" matchMax="1">metre</simpleAssociableChoice>
        <simpleAssociableChoice identifier="R3" matchMax="1">second</simpleAssociableChoice>
      </simpleMatchSet>
    </matchInteraction>"""),
    "items/q_text.xml": item21("text1", "Capital", decl("RESPONSE", "single", "string", ["Paris"], [("Paris", 1), ("paris", 1)]), """
    <p>The capital of France is <textEntryInteraction responseIdentifier="RESPONSE" expectedLength="10"/>.</p>"""),
    "items/q_order.xml": item21("order1", "Order", decl("RESPONSE", "ordered", "identifier", ["C", "A", "B"]), """
    <orderInteraction responseIdentifier="RESPONSE" shuffle="true">
      <prompt>Put these in order from smallest to largest.</prompt>
      <simpleChoice identifier="A">Cell</simpleChoice>
      <simpleChoice identifier="B">Organ</simpleChoice>
      <simpleChoice identifier="C">Atom</simpleChoice>
    </orderInteraction>"""),
    "items/q_inline.xml": item21("inline1", "Inline", decl("RESPONSE", "single", "identifier", ["Y"]), """
    <p>Water freezes at <inlineChoiceInteraction responseIdentifier="RESPONSE" shuffle="false">
      <inlineChoice identifier="X">10</inlineChoice><inlineChoice identifier="Y">0</inlineChoice></inlineChoiceInteraction> degrees Celsius.</p>"""),
    "items/q_extended.xml": item21("ext1", "Essay", decl("RESPONSE", "single", "string"), """
    <extendedTextInteraction responseIdentifier="RESPONSE" expectedLines="5">
      <prompt>Explain why the sky is blue.</prompt>
    </extendedTextInteraction>"""),
    "items/q_gap.xml": item21("gap1", "Gaps", decl("RESPONSE", "multiple", "directedPair", ["W1 G1", "W2 G2"]), """
    <gapMatchInteraction responseIdentifier="RESPONSE" shuffle="false">
      <prompt>Drag the words into the gaps.</prompt>
      <gapText identifier="W1" matchMax="1">oxygen</gapText>
      <gapText identifier="W2" matchMax="1">carbon dioxide</gapText>
      <p>Plants give off <gap identifier="G1"/> and take in <gap identifier="G2"/>.</p>
    </gapMatchInteraction>"""),
    "items/q_multi.xml": item21("multi1", "Two parts",
        decl("R1", "single", "identifier", ["T"]) + "\n" + decl("R2", "single", "string"), """
    <p>Read about the experiment, then answer both parts.</p>
    <choiceInteraction responseIdentifier="R1" maxChoices="1">
      <prompt>Was the result significant?</prompt>
      <simpleChoice identifier="T">True</simpleChoice><simpleChoice identifier="F">False</simpleChoice>
    </choiceInteraction>
    <extendedTextInteraction responseIdentifier="R2"><prompt>Explain your reasoning.</prompt></extendedTextInteraction>"""),
}

ORDER = ["items/q_choice.xml", "items/q_match.xml", "items/q_text.xml", "items/q_order.xml", "items/q_inline.xml", "items/q_extended.xml", "items/q_gap.xml", "items/q_multi.xml"]


def qti21_zip():
    refs = "\n".join(f'      <assessmentItemRef identifier="r{i}" href="{p}"/>' for i, p in enumerate(ORDER))
    test = f"""<?xml version="1.0" encoding="UTF-8"?>
<assessmentTest {NS} identifier="test1" title="Science check-in">
  <testPart identifier="part1" navigationMode="linear" submissionMode="individual">
    <assessmentSection identifier="s1" title="Section 1" visible="true">
{refs}
    </assessmentSection>
  </testPart>
</assessmentTest>
"""
    res = "\n".join(f'    <resource identifier="res{i}" type="imsqti_item_xmlv2p1" href="{p}"><file href="{p}"/></resource>' for i, p in enumerate(reversed(ORDER)))
    manifest = f"""<?xml version="1.0" encoding="UTF-8"?>
<manifest xmlns="http://www.imsglobal.org/xsd/imscp_v1p1" identifier="m1">
  <organizations/>
  <resources>
    <resource identifier="test" type="imsqti_test_xmlv2p1" href="assessment.xml"><file href="assessment.xml"/></resource>
{res}
  </resources>
</manifest>
"""
    files = [("imsmanifest.xml", manifest), ("assessment.xml", test)]
    files += [(p, ITEMS21[p]) for p in sorted(ITEMS21)]
    files.append(("images/flask.png", png(3, 2, (40, 120, 200))))
    write_zip("qti21_package.zip", files)


def moodle_image():
    import base64
    data = base64.b64encode(png(4, 3, (10, 160, 60))).decode()
    (HERE / "moodle_image.xml").write_text(f"""<?xml version="1.0" encoding="UTF-8"?>
<quiz>
  <question type="truefalse">
    <name><text>Leaf</text></name>
    <questiontext format="html">
      <text><![CDATA[<p>This leaf is green.</p><p><img src="@@PLUGINFILE@@/leaf%20photo.png" alt="leaf"></p>]]></text>
      <file name="leaf photo.png" path="/" encoding="base64">{data}</file>
    </questiontext>
    <answer fraction="100"><text>true</text></answer>
    <answer fraction="0"><text>false</text></answer>
  </question>
</quiz>
""")


def single_items():
    (HERE / "qti21_choice_multiple.xml").write_text(item21("ma1", "Metals", decl("RESPONSE", "multiple", "identifier", ["A", "C"]), """
    <p>Which of these are <b>metals</b>?</p>
    <choiceInteraction responseIdentifier="RESPONSE" shuffle="false" maxChoices="0">
      <simpleChoice identifier="A">Copper</simpleChoice>
      <simpleChoice identifier="B">Sulfur</simpleChoice>
      <simpleChoice identifier="C">Zinc</simpleChoice>
    </choiceInteraction>""", extra="3"))
    (HERE / "qti3_item.xml").write_text("""<?xml version="1.0" encoding="UTF-8"?>
<qti-assessment-item xmlns="http://www.imsglobal.org/xsd/imsqtiasi_v3p0" identifier="q3item" title="QTI 3 choice" adaptive="false" time-dependent="false">
  <qti-response-declaration identifier="RESPONSE" cardinality="single" base-type="identifier">
    <qti-correct-response><qti-value>choiceB</qti-value></qti-correct-response>
  </qti-response-declaration>
  <qti-outcome-declaration identifier="SCORE" cardinality="single" base-type="float"/>
  <qti-item-body>
    <p>What is the chemical formula of table salt?</p>
    <qti-choice-interaction response-identifier="RESPONSE" max-choices="1">
      <qti-simple-choice identifier="choiceA">KCl</qti-simple-choice>
      <qti-simple-choice identifier="choiceB">NaCl</qti-simple-choice>
      <qti-simple-choice identifier="choiceC">CaCO<sub>3</sub></qti-simple-choice>
    </qti-choice-interaction>
  </qti-item-body>
  <qti-response-processing template="https://purl.imsglobal.org/spec/qti/v3p0/rptemplates/match_correct.xml"/>
</qti-assessment-item>
""")


# ---------- Excel workbooks ----------

def col_name(i):
    s = ""
    i += 1
    while i:
        i, r = divmod(i - 1, 26)
        s = chr(65 + r) + s
    return s


def xlsx(name, sheets):
    """sheets: [(sheet name, rows)]; strings go to sharedStrings except cells given as
    ('inline', text); numbers stay numeric cells."""
    shared, index = [], {}
    sheet_xml = []
    for _, rows in sheets:
        out = []
        for r, row in enumerate(rows, start=1):
            cells = []
            for c, v in enumerate(row):
                ref = f"{col_name(c)}{r}"
                if v is None or v == "":
                    continue
                if isinstance(v, tuple):
                    cells.append(f'<c r="{ref}" t="inlineStr"><is><t>{escape(v[1])}</t></is></c>')
                elif isinstance(v, (int, float)):
                    cells.append(f'<c r="{ref}"><v>{v}</v></c>')
                else:
                    if v not in index:
                        index[v] = len(shared)
                        shared.append(v)
                    cells.append(f'<c r="{ref}" t="s"><v>{index[v]}</v></c>')
            out.append(f'<row r="{r}">{"".join(cells)}</row>')
        sheet_xml.append('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>' + "".join(out) + "</sheetData></worksheet>")
    sst = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="%d" uniqueCount="%d">' % (len(shared), len(shared))
    sst += "".join(f'<si><t xml:space="preserve">{escape(s)}</t></si>' for s in shared) + "</sst>"
    wb = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>'
    wb += "".join(f'<sheet name="{escape(n)}" sheetId="{i + 1}" r:id="rId{i + 1}"/>' for i, (n, _) in enumerate(sheets)) + "</sheets></workbook>"
    rels = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    rels += "".join(f'<Relationship Id="rId{i + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i + 1}.xml"/>' for i in range(len(sheets)))
    rels += f'<Relationship Id="rId{len(sheets) + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>'
    ct = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>'
    root_rels = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>'
    files = [("[Content_Types].xml", ct), ("_rels/.rels", root_rels), ("xl/workbook.xml", wb), ("xl/_rels/workbook.xml.rels", rels), ("xl/sharedStrings.xml", sst)]
    files += [(f"xl/worksheets/sheet{i + 1}.xml", x) for i, x in enumerate(sheet_xml)]
    write_zip(name, files)


def workbooks():
    xlsx("questions.xlsx", [
        ("Read me", [["This sheet has notes only."], ["The questions are on the next sheet."]]),
        ("Questions", [
            ["type", "stem", "a", "b", "c", "d", "correct", "correct_text", "points", "topic"],
            ["mc", "What is the largest planet?", "Mars", "Jupiter", "Venus", "Earth", "b", None, 2, "astronomy"],
            ["ma", "Which are gas giants?", "Jupiter", "Mars", "Saturn", None, "a,c", None, 1.5, "astronomy; planets"],
            ["tf", ("inline", "The Moon has no atmosphere to speak of."), None, None, None, None, None, "T", None, None],
            ["short", "Why do we have seasons?", None, None, None, None, None, "Earth's axis is tilted.", None, None],
            ["num", "How many planets are in the solar system?", None, None, None, None, None, 8, None, None],
            ["", "Default type is multiple choice: pick red.", "red", "blue", None, None, 1, None, None, None],
        ]),
    ])
    header = ["", "Question - max 120 characters", "Answer 1 - max 75 characters", "Answer 2 - max 75 characters", "Answer 3 - max 75 characters", "Answer 4 - max 75 characters", "Time limit (sec) - 5, 10, 20, 30, 60, 90, 120, or 240 secs", "Correct answer(s) - choose at least one"]
    xlsx("kahoot.xlsx", [("Sheet1", [
        ["Quiz template"], ["Add questions, at least two answer alternatives, time limit and choose correct answers."], [], [], [], [], [],
        header,
        [1, "What color is the sky on a clear day?", "Green", "Blue", "Red", "Yellow", 20, 2],
        [2, "Which are mammals?", "Whale", "Shark", "Bat", None, 30, "1,3"],
        [3, "The Earth is round.", "True", "False", None, None, 10, 1],
    ])])
    xlsx("vocab.xlsx", [("Terms", [
        ["Term", "Definition"],
        ["evaporation", "liquid turning into a gas"],
        ["condensation", "gas turning into a liquid"],
        ["precipitation", "water falling from clouds"],
    ])])


if __name__ == "__main__":
    canvas_zip()
    qti21_zip()
    single_items()
    moodle_image()
    workbooks()
    print("fixtures written to", HERE)
