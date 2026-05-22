/**
 * Scoring Configuration & Functions
 *
 * Extracts all scoring formulas, boosts, dampening factors, and
 * threshold constants from the ContextBuilder into a single configuration object.
 * This eliminates magic numbers scattered across 500+ lines of findRelevantContext.
 */

import type { Node, NodeKind } from '../types';

// ── Score Configuration ──

export interface ScoreConfig {
  // Exact match channel
  exactMatch: {
    coLocationBoostPerSymbol: number;
    maxResultsMultiplier: number;
  };
  // Definition prefix search
  definitionPrefix: {
    brevityBonusScale: number;
    brevityBonusCap: number;
    resultsPerSymbol: number;
  };
  // Text/FTS channel
  textSearch: {
    termHitBoostPerHit: number;
    dampenNonTestInTestFile: number;
  };
  // Multi-term co-occurrence
  multiTerm: {
    dampenSingleTerm: number;
    multiplicativeBoostPair: number;
    multiplicativeBoostTriple: number;
  };
  // CamelCase-boundary LIKE
  camelCase: {
    brevityBonusCap: number;
    brevityBonusScale: number;
    resultsPerSymbol: number;
  };
  // Compound term
  compoundTerm: {
    baseScore: number;
    scorePerExtraTerm: number;
    resultsLimit: number;
  };
  // Path relevance
  pathRelevance: {
    filenameExact: number;
    directoryExact: number;
    generalMatch: number;
  };
  // Kind priority bonus
  kindPriority: Record<NodeKind, number>;
  // Name match bonus
  nameMatch: {
    exact: number;
    tokenExact: number;
    allTerms: number;
    substring: number;
    prefixMinLen: number;
    prefixMaxLen: number;
    prefixScoreRange: number;
  };
  // Post-processing
  postProcess: {
    perFileDiversityRatio: number;
    nonProductionCapRatio: number;
    testFileScoreDampen: number;
  };
}

export const DEFAULT_SCORE_CONFIG: ScoreConfig = {
  exactMatch: {
    coLocationBoostPerSymbol: 20,
    maxResultsMultiplier: 3,
  },
  definitionPrefix: {
    brevityBonusScale: 3,
    brevityBonusCap: 10,
    resultsPerSymbol: 5,
  },
  textSearch: {
    termHitBoostPerHit: 5,
    dampenNonTestInTestFile: 0.3,
  },
  multiTerm: {
    dampenSingleTerm: 0.6,
    multiplicativeBoostPair: 2.0,
    multiplicativeBoostTriple: 2.5,
  },
  camelCase: {
    brevityBonusCap: 6,
    brevityBonusScale: 4,
    resultsPerSymbol: 3,
  },
  compoundTerm: {
    baseScore: 10,
    scorePerExtraTerm: 20,
    resultsLimit: 3,
  },
  pathRelevance: {
    filenameExact: 10,
    directoryExact: 5,
    generalMatch: 3,
  },
  kindPriority: {
    file: 0,
    module: 2,
    class: 8,
    struct: 7,
    interface: 7,
    trait: 7,
    protocol: 7,
    function: 10,
    method: 10,
    property: 4,
    field: 4,
    variable: 5,
    constant: 6,
    enum: 6,
    enum_member: 0,
    type_alias: 6,
    namespace: 3,
    parameter: 0,
    import: 0,
    export: 0,
    route: 9,
    component: 8,
  },
  nameMatch: {
    exact: 80,
    tokenExact: 60,
    allTerms: 15,
    substring: 10,
    prefixMinLen: 3,
    prefixMaxLen: 8,
    prefixScoreRange: 30,
  },
  postProcess: {
    perFileDiversityRatio: 0.20,
    nonProductionCapRatio: 0.15,
    testFileScoreDampen: 0.3,
  },
};

// ── Scoring Functions ──

/**
 * Compute a kind-priority bonus for a node.
 */
export function kindBonus(node: Node, config: ScoreConfig = DEFAULT_SCORE_CONFIG): number {
  return config.kindPriority[node.kind as NodeKind] ?? 0;
}

/**
 * Compute a name-match bonus based on how closely the query symbol
 * matches the node name.
 *
 * @param querySymbol - Extracted symbol from the query
 * @param nodeName - Node's name field
 * @param config - Scoring configuration
 */
export function nameMatchBonus(
  querySymbol: string,
  nodeName: string,
  config: ScoreConfig = DEFAULT_SCORE_CONFIG
): number {
  const nm = config.nameMatch;
  const lowerName = nodeName.toLowerCase();

  // Exact match
  if (lowerName === querySymbol.toLowerCase()) {
    return nm.exact;
  }

  // Token exact (CamelCase/snake_case boundary match)
  const tokens = querySymbol
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/[_\-.\s]+/g, ' ')
    .toLowerCase()
    .split(/\s+/);
  for (const token of tokens) {
    if (lowerName.includes(token)) {
      return nm.tokenExact;
    }
  }

  // All terms present
  const allTermsMatch = tokens.every(t => lowerName.includes(t));
  if (allTermsMatch && tokens.length > 1) {
    return nm.allTerms;
  }

  // Substring match
  if (lowerName.includes(querySymbol.toLowerCase())) {
    return nm.substring;
  }

  // Prefix match (scaled by query length)
  const qlen = querySymbol.length;
  if (qlen >= nm.prefixMinLen && lowerName.startsWith(querySymbol.toLowerCase())) {
    const scale = Math.min((qlen - nm.prefixMinLen) / (nm.prefixMaxLen - nm.prefixMinLen), 1.0);
    return Math.round(scale * nm.prefixScoreRange) + nm.prefixMinLen;
  }

  return 0;
}

/**
 * Compute brevity bonus: shorter names matching longer query terms
 * get a boost.
 *
 * @param nameLen - Length of node name
 * @param titleLen - Length of title-cased query symbol
 * @param config - Scoring configuration (uses definitionPrefix section)
 */
export function brevityBonus(
  nameLen: number,
  titleLen: number,
  config: ScoreConfig = DEFAULT_SCORE_CONFIG
): number {
  const dp = config.definitionPrefix;
  return Math.max(0, dp.brevityBonusCap - Math.floor((nameLen - titleLen) / dp.brevityBonusScale));
}

/**
 * Score path relevance: how closely a node's file path matches a query term.
 *
 * @param filePath - Node's file path
 * @param term - Query term (lowercased)
 * @param config - Scoring configuration
 */
export function scorePathRelevance(
  filePath: string,
  term: string,
  config: ScoreConfig = DEFAULT_SCORE_CONFIG
): number {
  const pr = config.pathRelevance;
  const lower = filePath.toLowerCase();
  const fileName = lower.split(/[/\\]/).pop() || '';

  if (fileName === term || fileName.startsWith(term + '.')) return pr.filenameExact;
  if (lower.includes('/' + term + '/') || lower.includes('\\' + term + '\\')) return pr.directoryExact;
  if (lower.includes(term)) return pr.generalMatch;
  return 0;
}

/**
 * Co-location boost: when multiple query symbols exist in the same file,
 * score gets a proportional boost.
 *
 * @param symbolCount - Number of matched symbols in the file
 * @param config - Scoring configuration
 */
export function coLocationBoost(
  symbolCount: number,
  config: ScoreConfig = DEFAULT_SCORE_CONFIG
): number {
  if (symbolCount <= 1) return 0;
  return config.exactMatch.coLocationBoostPerSymbol * (symbolCount - 1);
}

/**
 * Dampen score for test files when the query isn't test-related.
 */
export function testFileDampen(
  score: number,
  isTestFile: boolean,
  isTestQuery: boolean,
  config: ScoreConfig = DEFAULT_SCORE_CONFIG
): number {
  if (isTestFile && !isTestQuery) {
    return score * config.postProcess.testFileScoreDampen;
  }
  return score;
}

/**
 * Multi-term boost: when multiple query terms match, apply multiplicative boost.
 */
export function multiTermBoost(
  matchCount: number,
  config: ScoreConfig = DEFAULT_SCORE_CONFIG
): number {
  if (matchCount >= 3) return config.multiTerm.multiplicativeBoostTriple;
  if (matchCount >= 2) return config.multiTerm.multiplicativeBoostPair;
  return 1.0;
}

/**
 * CamelCase multi-term accumulation score for LIKE-based boundary matches.
 * A node matching 3+ query terms in its name (e.g., ExtensionHostProcess)
 * gets aggressive scaling.
 */
export function camelCaseMultiTermScore(
  score: number,
  termCount: number
): number {
  return score * (1 + termCount) + (termCount - 1) * 30;
}
