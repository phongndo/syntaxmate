/**
 * @name Basic Unicode query — café 東京 🚀
 * @description Exercises declarations, quantifiers, and select scopes.
 */
import semmle.code.cpp.dataflow.DataFlow

class NamedNode extends DataFlow::Node {
  NamedNode() { exists(string label | label = "λ-node" and this.toString() = label) }

  string getLabel() { result = "café 🚀" }
}

bindingset[value]
private predicate interesting(int value, string text) {
  value >= 0 and value < 42 and text != ""
}

from NamedNode node, int score
where interesting(score, node.getLabel()) or score = -1
select node, node.getLabel() + " — 東京", score as rank
