# Extract the person columns needed for education targets from ACS 2023 1-year PUMS.
import subprocess, pandas as pd
cols = ["SERIALNO", "SPORDER", "STATE", "RELSHIPP", "AGEP", "SEX", "SCH", "SCHG", "SCHL", "RAC1P", "HISP", "NATIVITY", "MAR", "PWGTP"]
parts = []
for f in ("psam_pusa.csv", "psam_pusb.csv"):
    p = subprocess.Popen(["unzip", "-p", "/home/player1/internot/datasets/acs/pums2023_1yr/csv_pus.zip", f], stdout=subprocess.PIPE)
    parts.append(pd.read_csv(p.stdout, usecols=cols, dtype={"SERIALNO": str}))
    p.wait()
df = pd.concat(parts, ignore_index=True)
df.to_parquet("pums_edu.parquet")
print(len(df), df.dtypes.to_dict())
