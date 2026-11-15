import json,tempfile,unittest
from pathlib import Path
from company_research import fresh_access,RESEARCH_TOOLS
ROOT=Path(__file__).resolve().parents[1]
class ResearchDefaults(unittest.TestCase):
 def test_every_configured_role_receives_research_without_other_roles_work_tools(self):
  config=json.loads((ROOT/'config/company.local.json').read_text());actors=[a['id'] for a in config['agents']]
  access,proof=fresh_access(ROOT/'config/company-research.json',actors,'Engineer Gent',['files:write','finish'])
  self.assertEqual(len(access),6);self.assertTrue(proof['enabled'])
  for actor in actors:
   self.assertEqual(set(access[actor]['tools']) & RESEARCH_TOOLS,RESEARCH_TOOLS)
   self.assertEqual(access[actor]['memory_from'],[])
   if actor!='Engineer Gent':self.assertEqual(set(access[actor]['tools']),RESEARCH_TOOLS)
  self.assertIn('files:write',access['Engineer Gent']['tools']);self.assertEqual(len(proof['profile_sha256']),64)
 def test_defaults_cannot_introduce_execution_or_external_write_capabilities(self):
  for forbidden in ['shell','source_repair','email_send','model_download','access_delegate','files:write']:
   with tempfile.TemporaryDirectory() as d:
    p=Path(d)/'profile.json';p.write_text(json.dumps({'version':1,'enabled':True,'tools':[forbidden]}))
    with self.assertRaises(ValueError):fresh_access(p,['A'],'A',[])
 def test_disable_is_owner_controlled_and_keeps_exact_task_tools(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'profile.json';p.write_text(json.dumps({'version':1,'enabled':False,'tools':['web_read']}));a,proof=fresh_access(p,['A','B'],'A',['files:read'])
   self.assertEqual(a['A']['tools'],['files:read']);self.assertEqual(a['B']['tools'],[]);self.assertFalse(proof['enabled'])
 def test_unknown_actor_and_malformed_enable_are_rejected(self):
  p=ROOT/'config/company-research.json'
  for actors,primary in [(['A'],'B'),(['A','A'],'A'),([], 'A')]:
   with self.assertRaises(ValueError):fresh_access(p,actors,primary,[])
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'profile.json';p.write_text(json.dumps({'version':1,'enabled':'true','tools':[]}))
   with self.assertRaises(ValueError):fresh_access(p,['A'],'A',[])
 def test_research_catalog_has_real_implemented_tool_contracts(self):
  catalog=json.loads((ROOT/'config/company-tools.json').read_text());inputs=json.loads((ROOT/'config/company-tool-inputs.json').read_text())
  self.assertTrue(RESEARCH_TOOLS<=catalog.keys());self.assertTrue(RESEARCH_TOOLS<=inputs.keys())
if __name__=='__main__':unittest.main()
