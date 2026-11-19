import io,json,pathlib,unittest
from unittest import mock
import showcase_observer as m
import showcase_observer_company_tests as own
NAMES=['test_evidence_store_preserves_mtime_and_notices_external_changes', 'test_evidence_store_atomic_replace_failure_keeps_original_and_cleans_temp', 'test_terminal_run_counts_calls_and_reports_zero_cost', 'test_timeout_preserves_running_runtime_status', 'test_pending_attention_returns_attention_and_answered_attention_continues', 'test_rejects_invalid_costs_and_unsafe_paths']
class Quality(unittest.TestCase):
 def run_target(self,name):
  result=unittest.TextTestRunner(stream=io.StringIO()).run(own.ShowcaseObserverTests(name))
  return result
 def test_real_core_and_all_company_methods_pass(self):
  self.assertEqual(sorted(unittest.defaultTestLoader.getTestCaseNames(own.ShowcaseObserverTests)),sorted(NAMES))
  for name in NAMES:
   result=self.run_target(name)
   self.assertTrue(result.wasSuccessful(),name)
   self.assertFalse(result.skipped,name)
 def test_company_methods_detect_frozen_regressions(self):
  original_observe=m.observe_run
  def bad_attention(*a,**kw):
   result=original_observe(*a,**kw)
   if result['status']=='attention': result['runtime_status']='attention'
   return result
  def bad_terminal(*a,**kw):
   result=original_observe(*a,**kw)
   if result['runtime_status']=='blocked':result['runtime_status']='running';result['status']='observation_expired'
   return result
  original_summary=m._summarize_snapshot
  def bad_count(snapshot):
   status,count,cost=original_summary(snapshot);return status,count+1,cost
  original_write=m.EvidenceStore.write_if_changed
  seen=set()
  def stale_cache(store,name,value):
   key=(id(store),name)
   if key in seen:return False
   seen.add(key);return original_write(store,name,value)
  def bad_atomic(store,name,value):
   target=store._directory/name;target.write_text(json.dumps(value));raise OSError('mutant replacement failed after overwrite')
  original_name=m._validate_evidence_name
  def bad_names(name):
   if '..' in name:raise ValueError('mutant rejects ordinary basename')
   return original_name(name)
  cases=[(m,'_extract_cost',lambda reply:0.0,NAMES[5]),
         (m,'_summarize_snapshot',bad_count,NAMES[2]),
         (m,'observe_run',bad_terminal,NAMES[2]),
         (m,'observe_run',bad_attention,NAMES[4]),
         (m.EvidenceStore,'write_if_changed',stale_cache,NAMES[0]),
         (m.EvidenceStore,'write_if_changed',bad_atomic,NAMES[1]),
         (m,'_validate_evidence_name',bad_names,NAMES[5])]
  for obj,attribute,mutant,name in cases:
   with self.subTest(regression=attribute+':'+name),mock.patch.object(obj,attribute,mutant):
    result=self.run_target(name)
    self.assertFalse(result.wasSuccessful(),'Company test did not detect '+attribute+':'+name)
    self.assertFalse(result.skipped,'Skipped mutant is not detection')
