# Stream the CCD 2024-25 membership file (2.3 GB CSV inside a deflate64 zip, which
# Python's zipfile can't read) from stdin and keep per-school totals and
# per-school per-grade totals. Run:
#   unzip -p datasets/education/ccd/ccd_sch_052_2425_l_1a_073025.zip ccd_sch_052_2425_l_1a_073025.csv | python3 membership.py
import csv, io, sys
out_g = open("/home/player1/internot/datasets/education/ccd/derived_membership_by_grade_2425.csv", "w", newline="")
out_t = open("/home/player1/internot/datasets/education/ccd/derived_membership_total_2425.csv", "w", newline="")
wg = csv.writer(out_g); wg.writerow(["NCESSCH", "GRADE", "STUDENT_COUNT"])
wt = csv.writer(out_t); wt.writerow(["NCESSCH", "STUDENT_COUNT", "DMS_FLAG"])
if True:
    f = sys.stdin.buffer
    r = csv.reader(io.TextIOWrapper(f, encoding="latin-1"))
    h = next(r)
    i_id, i_g, i_c, i_t, i_f = (h.index(k) for k in ("NCESSCH", "GRADE", "STUDENT_COUNT", "TOTAL_INDICATOR", "DMS_FLAG"))
    for row in r:
        t = row[i_t]
        if t == "Subtotal 4 - By Grade":
            wg.writerow([row[i_id], row[i_g], row[i_c]])
        elif t == "Education Unit Total":
            wt.writerow([row[i_id], row[i_c], row[i_f]])
out_g.close(); out_t.close()
print("done")
