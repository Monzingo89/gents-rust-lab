export const basicAnswers = [
  ['Is Gents free?', 'Original Gents code uses the MIT license. Fully local task runs can have $0 model-provider and tool fees. Your computer, electricity, storage and development time still have costs. Model weights and other components keep their own licenses.'],
  ['Does my request leave my computer?', 'This introduction page does not send requests or run agents. The default local launcher keeps model inference on your computer. Downloads and optional integrations require separate configuration; review permissions before enabling them.'],
  ['Can it finish any task?', 'No. Current evidence covers bounded synthetic tasks and specific local workflows. Unsupported tools, hardware limits and missing acceptance checks can stop a task. Human review remains necessary.'],
  ['Which computer should I use?', 'Start with the time scenario calculator and measure your own model speeds. Available memory, model size and the tools a task needs matter. The existing code-execution sandbox is verified on macOS; Linux and Windows support varies by tool.'],
  ['What does the CEO do?', 'The local CEO asks about your outcome and definition of done, then proposes work and tools. You review the task and permissions. The full guided conversation-to-task setup is still being developed.'],
  ['Are banking and health tasks certified?', 'No. There is no SOC attestation or blanket health, banking or fintech compliance claim. Use synthetic examples while control requirements, operational evidence and domain-specific reviews remain incomplete.'],
  ['Can I use it offline?', 'Installed local models and supported local tools can operate without paid inference services. Initial dependencies or model downloads need a connection unless already available. Optional integrations have their own network requirements.'],
  ['Is this a hosted AI service?', 'No. This site introduces the free local project. It does not expose your local company, task files, memory or tools to the public internet.']
];

// Known project answers only. Unknown or ambiguous questions are never guessed.
const normalize = text => typeof text === 'string' && text.length <= 300
 ? text.toLowerCase().replace(/[^a-z0-9 ]/g, ' ').replace(/\s+/g, ' ').trim() : '';
const aliases = [
 ['is it free','how much does it cost','is metivolve free','what does it cost'],
 ['is my data private','does my data leave my computer','where does my data go'],
 ['can it do anything','can it do every task'],
 ['what computer do i need','will my computer work','what gpu do i need'],
 ['what is the ceo','how does the ceo work'],
 ['is it hipaa compliant','is it soc compliant','is it compliant'],
 ['does it work offline','can i work offline'],
 ['does this website run ai','does this site run agents','is it hosted']
];
export function basicAnswer(question) {
 const value=normalize(question);if(!value)return null;
 const index=basicAnswers.findIndex(([q],i)=>[q,...aliases[i]].some(alias=>normalize(alias)===value));
 return index<0?null:{question:basicAnswers[index][0],answer:basicAnswers[index][1],source:'Project setup guide',modelCalls:0,toolCalls:0};
}
export function mountBasicAnswers(root) {
 const heading=document.createElement('h3');heading.textContent='Quick setup answers';
 const note=document.createElement('p');note.textContent='Choose a common question for an immediate answer. No model, tools or network requests.';
 const label=document.createElement('label'),select=document.createElement('select');label.textContent='Your question';
 const empty=document.createElement('option');empty.value='';empty.textContent='Choose a question';select.append(empty);
 basicAnswers.forEach(([q],i)=>{const option=document.createElement('option');option.value=String(i);option.textContent=q;select.append(option);});
 const answer=document.createElement('p');answer.setAttribute('role','status');answer.dataset.basicAnswer='';
 select.addEventListener('change',()=>{answer.textContent=select.value===''?'':basicAnswers[Number(select.value)][1];});
 label.append(select);root.append(heading,note,label,answer);root.dataset.basicAnswersReady='true';
}
