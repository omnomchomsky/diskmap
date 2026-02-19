const { scanOneDir } = require('./scanner');
const { Tree } = require('./tree');

class AsyncQueue {
  constructor() {
    this.items = [];
    this.waiters = [];
    this.closed = false;
  }

  push(item) {
    if (this.closed) {
      return false;
    }
    if (this.waiters.length > 0) {
      const resolve = this.waiters.shift();
      resolve({ value: item, done: false });
      return true;
    }
    this.items.push(item);
    return true;
  }

  close() {
    this.closed = true;
    while (this.waiters.length > 0) {
      const resolve = this.waiters.shift();
      resolve({ done: true });
    }
  }

  async shift() {
    if (this.items.length > 0) {
      return { value: this.items.shift(), done: false };
    }
    if (this.closed) {
      return { done: true };
    }
    return new Promise((resolve) => {
      this.waiters.push(resolve);
    });
  }
}

class Session {
  constructor(rootPath, topK) {
    const rootName = rootPath;
    this.tree = new Tree(rootName, topK);
    this.queue = [
      {
        path: rootPath,
        nodeId: 0,
        depth: 0,
      },
    ];
    this.topK = topK;
    this.errors = 0;
    this.jobsStarted = 0;
    this.jobsDone = 0;
  }

  async run(fsAdapter) {
    while (this.queue.length > 0) {
      const job = this.queue.shift();
      this.jobsStarted += 1;
      await scanOneDir(fsAdapter, job, (event) => {
        this.applyEvent(event);
      });
    }
  }

  async runParallel(fsAdapter, numThreads) {
    const jobQueue = new AsyncQueue();
    const eventQueue = new AsyncQueue();

    while (this.queue.length > 0) {
      const job = this.queue.shift();
      jobQueue.push(job);
      this.jobsStarted += 1;
    }

    let activeJobs = this.jobsStarted;

    const workers = [];
    for (let i = 0; i < numThreads; i += 1) {
      workers.push(
        (async () => {
          while (true) {
            const { value: job, done } = await jobQueue.shift();
            if (done) {
              break;
            }
            await scanOneDir(fsAdapter, job, (event) => {
              eventQueue.push(event);
            });
          }
        })()
      );
    }

    while (activeJobs > 0) {
      const { value: event, done } = await eventQueue.shift();
      if (done) {
        break;
      }

      if (event.type === 'Dir') {
        const childName = event.name.toString();
        const childId = this.tree.addChild(event.parentId, childName, this.topK);
        const newJob = {
          path: event.path,
          nodeId: childId,
          depth: 0,
        };
        jobQueue.push(newJob);
        this.jobsStarted += 1;
        activeJobs += 1;
        this.tree.nodeMut(event.parentId).markPartial();
      } else if (event.type === 'Done') {
        this.jobsDone += 1;
        activeJobs -= 1;
      }

      if (event.type !== 'Dir') {
        this.applyEvent(event);
      }
    }

    jobQueue.close();
    eventQueue.close();
    await Promise.all(workers);
  }

  async runParallelWithCallback(fsAdapter, numThreads, callback) {
    const jobQueue = new AsyncQueue();
    const eventQueue = new AsyncQueue();

    while (this.queue.length > 0) {
      const job = this.queue.shift();
      jobQueue.push(job);
      this.jobsStarted += 1;
    }

    let activeJobs = this.jobsStarted;

    const workers = [];
    for (let i = 0; i < numThreads; i += 1) {
      workers.push(
        (async () => {
          while (true) {
            const { value: job, done } = await jobQueue.shift();
            if (done) {
              break;
            }
            await scanOneDir(fsAdapter, job, (event) => {
              eventQueue.push(event);
            });
          }
        })()
      );
    }

    const intervalMs = 200;
    const timer = setInterval(() => {
      callback(this);
    }, intervalMs);

    while (activeJobs > 0) {
      const { value: event, done } = await eventQueue.shift();
      if (done) {
        break;
      }

      if (event.type === 'Dir') {
        const childName = event.name.toString();
        const childId = this.tree.addChild(event.parentId, childName, this.topK);
        const newJob = {
          path: event.path,
          nodeId: childId,
          depth: 0,
        };
        jobQueue.push(newJob);
        this.jobsStarted += 1;
        activeJobs += 1;
        this.tree.nodeMut(event.parentId).markPartial();
      } else if (event.type === 'Done') {
        this.jobsDone += 1;
        activeJobs -= 1;
      }

      if (event.type !== 'Dir') {
        this.applyEvent(event);
      }
    }

    clearInterval(timer);
    callback(this);

    jobQueue.close();
    eventQueue.close();
    await Promise.all(workers);
  }

  applyEvent(event) {
    switch (event.type) {
      case 'File': {
        const node = this.tree.nodeMut(event.parentId);
        node.addFile(event.name.toString(), event.size);

        const parentId = this.tree.node(event.parentId).parentId();
        if (parentId !== null && parentId !== undefined) {
          this.propagateKnownDelta(parentId, event.size);
        }
        break;
      }
      case 'Dir': {
        const childName = event.name.toString();
        const childId = this.tree.addChild(event.parentId, childName, this.topK);
        this.queue.push({
          path: event.path,
          nodeId: childId,
          depth: 0,
        });
        this.tree.nodeMut(event.parentId).markPartial();
        break;
      }
      case 'Done': {
        this.jobsDone += 1;
        this.tree.nodeMut(event.node).markComplete();
        break;
      }
      case 'Error': {
        this.errors += 1;
        this.tree.nodeMut(event.node).markPartial();
        break;
      }
      default:
        break;
    }
  }

  propagateKnownDelta(fromId, delta) {
    let current = fromId;
    while (current !== null && current !== undefined) {
      const parent = this.tree.node(current).parentId();
      this.tree.nodeMut(current).addSubtreeKnown(delta);
      current = parent;
    }
  }
}

module.exports = {
  Session,
};
