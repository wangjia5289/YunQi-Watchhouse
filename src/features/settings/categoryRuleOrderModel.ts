export function moveCategoryRuleId(
  ruleIds: number[],
  movingId: number,
  targetId: number,
): number[] {
  const movingIndex = ruleIds.indexOf(movingId);
  const targetIndex = ruleIds.indexOf(targetId);
  if (movingIndex < 0 || targetIndex < 0 || movingIndex === targetIndex) return ruleIds;

  const reordered = [...ruleIds];
  reordered.splice(movingIndex, 1);
  reordered.splice(targetIndex, 0, movingId);
  return reordered;
}

export function offsetCategoryRuleId(
  ruleIds: number[],
  ruleId: number,
  offset: -1 | 1,
): number[] {
  const index = ruleIds.indexOf(ruleId);
  const targetIndex = index + offset;
  if (index < 0 || targetIndex < 0 || targetIndex >= ruleIds.length) return ruleIds;
  return moveCategoryRuleId(ruleIds, ruleId, ruleIds[targetIndex]);
}
