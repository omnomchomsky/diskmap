const { TopKFiles, clampToSafeInteger } = require('./top_files');

const NodeState = Object.freeze({
  Unseen: 'Unseen',
  Partial: 'Partial',
  Complete: 'Complete',
});

function saturatingAdd(a, b) {
  return clampToSafeInteger(a + b);
}

class Node {
  constructor(id, parent, name, topK) {
    this.id = id;
    this.parent = parent;
    this.name = name;
    this.children = [];
    this.own_files_bytes = 0;
    this.own_files_count = 0;
    this.subtree_bytes_known = 0;
    this.subtree_bytes_unknown = 0;
    this.state = NodeState.Unseen;
    this.top_files = new TopKFiles(topK);
  }

  markPartial() {
    if (this.state === NodeState.Unseen) {
      this.state = NodeState.Partial;
    }
  }

  markComplete() {
    this.state = NodeState.Complete;
    this.subtree_bytes_unknown = 0;
  }

  addFile(name, size) {
    this.own_files_bytes = saturatingAdd(this.own_files_bytes, size);
    this.own_files_count = saturatingAdd(this.own_files_count, 1);
    this.top_files.offer(name, size);
    this.markPartial();
  }

  addSubtreeKnown(delta) {
    this.subtree_bytes_known = saturatingAdd(this.subtree_bytes_known, delta);
    this.markPartial();
  }

  parentId() {
    return this.parent;
  }

  idValue() {
    return this.id;
  }

  nameValue() {
    return this.name;
  }

  childrenValue() {
    return this.children;
  }

  totalBytes() {
    return this.own_files_bytes + this.subtree_bytes_known;
  }

  ownBytes() {
    return this.own_files_bytes;
  }

  subtreeBytes() {
    return this.subtree_bytes_known;
  }

  topFiles() {
    return this.top_files;
  }
}

module.exports = {
  Node,
  NodeState,
};
