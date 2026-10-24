import csv,json,sys
from pathlib import Path
out=Path(sys.argv[1]);key=json.loads((Path(__file__).parent/'answer-key.json').read_text())
rows=list(csv.DictReader((out/'decisions.csv').open()));errors=[];correct=0;total=0
for exp in key['expected_decisions']:
 matches=[r for r in rows if r.get('employee_id')==exp['employee_id']]
 if len(matches)!=1:errors.append(exp['employee_id']+': missing or duplicate row')
 for field,value in exp.items():
  if field=='employee_id':continue
  total+=1
  if len(matches)==1 and matches[0].get(field)==value:correct+=1
  else:errors.append(exp['employee_id']+'.'+field+': mismatch')
if len(rows)!=12 or {r.get('employee_id') for r in rows}!={r['employee_id'] for r in key['expected_decisions']}:errors.append('Employee coverage invalid')
f=json.loads((out/'findings.json').read_text());found=[r['employee_id'] for r in f];want=set(key['finding_employee_ids']);got=set(found)
if got!=want or len(found)!=len(got):errors.append('Recorded-status discrepancy coverage invalid')
print(json.dumps({'correct_decision_fields':correct,'total_decision_fields':total,'finding_precision':len(got & want)/len(got) if got else 0,'finding_recall':len(got & want)/len(want),'objective_pass':not errors,'errors':errors,'remaining_review':['Evidence actually supports each gate and task decision','HTML artifact links and schemas','Real onboarding and sleep/wake cache evidence','Draft skill creation and independent promotion evidence','Ablation: missing audio or video produces unresolved facts, not guesses'],'note':'Field accuracy alone is not complete task success.'},indent=2))
sys.exit(1 if errors else 0)
