/**
 * cross-component で共有する小さな utility 群。
 *
 * - `isInProject`: 与えられた path が project root 配下かを大文字小文字 + path
 *   separator 非依存で判定。 root が null の場合は「project が未確定」とみなして
 *   true を返す (= グラフ上の node を pre-mark しない、 safe default)。
 */

/**
 * path が project root 配下かを判定する。
 *
 * - root が null/空: 判定不能 → true (= フィルタリング無効、 safe default)
 * - 大文字小文字を無視 (Windows 大文字小文字非区別の volume を想定)
 * - `\` と `/` を統一して比較
 */
export function isInProject(path: string, root: string | null): boolean {
  if (!root) return true;
  const norm = (s: string) => s.replace(/\\/g, "/").toLowerCase();
  return norm(path).startsWith(norm(root));
}
