const { Node } = require('./model');

class Tree {
  constructor(rootName, topK) {
    this.root = 0;
    this.nodes = [new Node(0, null, rootName, topK)];
  }

  addChild(parentId, name, topK) {
    const id = this.nodes.length;
    this.nodes.push(new Node(id, parentId, name, topK));
    this.nodes[parentId].children.push(id);
    return id;
  }

  node(id) {
    return this.nodes[id];
  }

  nodeMut(id) {
    return this.nodes[id];
  }

  rootId() {
    return this.root;
  }
}

module.exports = {
  Tree,
};
