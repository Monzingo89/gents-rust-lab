// Fixed host acceptance, independent of the local author's implementation.
mod candidate {include!(env!("GENTS_CANDIDATE_SOURCE"));}
fn expected(text:&str,offset:usize,limit:usize)->Option<usize>{
 if limit==0||offset>text.len()||!text.is_char_boundary(offset){return None;}
 let ceiling=offset.saturating_add(limit).min(text.len());
 let end=(offset..=ceiling).rev().find(|&i|text.is_char_boundary(i))?;
 if end==offset&&offset<text.len(){None}else{Some(end)}
}
#[test]
fn exhaustive_unicode_offsets_limits_and_overflow_match_fixed_contract(){
 for text in ["","a","α","αβ","A🧪B","日本語","e\u{301}","line\nnext"]{
  for offset in 0..=text.len()+2{
   for limit in (0..=text.len()+2).chain([usize::MAX-1,usize::MAX]){
    assert_eq!(candidate::bounded_chunk_end(text,offset,limit),expected(text,offset,limit),"text={text:?},offset={offset},limit={limit}");
   }
  }
 }
}
