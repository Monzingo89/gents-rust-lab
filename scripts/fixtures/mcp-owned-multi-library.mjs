// v1 is retained verbatim with its original frozen hash and failed attempt.
// Prepared custody stages package-canonical paths; fix benchmark aliases only.
import original from './mcp-owned-multi-library-v1.mjs';
export default {
 ...original,
 ownedComponents:original.ownedComponents.map(spec=>({...spec,source_path:`owned-${spec.owner_declared_review_package_id}.rs`})),
 files:original.files.map(file=>({...file,path:`owned-${file.owned_component}.rs`}))
};
