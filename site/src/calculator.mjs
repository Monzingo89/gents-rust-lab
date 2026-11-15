// Scenario math only; never invent a device's throughput or predict task success.
export function estimate({ inputTokens, outputTokens, prefillRate, generationRate, overheadSeconds = 0 }) {
  const values = [inputTokens, outputTokens, prefillRate, generationRate, overheadSeconds].map(Number);
  if ([inputTokens, outputTokens, prefillRate, generationRate].some(v => String(v).trim() === '') ||
      values.some(v => !Number.isFinite(v)) || values[0] < 0 || values[1] < 0 ||
      values[2] <= 0 || values[3] <= 0 || values[4] < 0) return null;
  const seconds = values[0] / values[2] + values[1] / values[3] + values[4];
  return Number.isFinite(seconds) ? { seconds, providerToolFeesUsd: 0 } : null;
}
export function duration(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return 'Unknown';
  if (seconds < 60) return `${Math.ceil(seconds)} seconds`;
  if (seconds < 3600) return `${(seconds / 60).toFixed(1)} minutes`;
  return `${(seconds / 3600).toFixed(1)} hours`;
}

// Owner-entered scenarios only. No current API/GPU/electricity prices assumed.
export function ownership({hardwareCost, monthlyApiSpend, powerWatts, monthlyHours, electricityPriceKwh}) {
  const raw=[hardwareCost,monthlyApiSpend,powerWatts,monthlyHours,electricityPriceKwh];
  if(raw.some(v=>String(v ?? '').trim()===''))return null;
  const [capital,api,watts,hours,rate]=raw.map(Number);
  if([capital,api,watts,hours,rate].some(v=>!Number.isFinite(v)||v<0)||hours>744)return null;
  const electricity=watts/1000*hours*rate;
  const savings=api-electricity;
  const breakEvenMonths=savings>0?capital/savings:null;
  if(![electricity,savings].every(Number.isFinite)||(breakEvenMonths!==null&&!Number.isFinite(breakEvenMonths)))return null;
  return {monthlyElectricity:electricity,monthlySavings:savings,breakEvenMonths,providerToolFeesUsd:0};
}
