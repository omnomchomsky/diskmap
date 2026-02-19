const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('__TAURI__', {
  core: {
    invoke: (cmd, args) => ipcRenderer.invoke('tauri:invoke', { cmd, args }),
    platform: process.platform,
  },
  dialog: {
    open: (options) => ipcRenderer.invoke('dialog:open', options),
  },
  event: {
    listen: (eventName, handler) => {
      const channel = `event:${eventName}`;
      const listener = (_event, payload) => handler({ payload });
      ipcRenderer.on(channel, listener);
      return Promise.resolve(() => {
        ipcRenderer.removeListener(channel, listener);
      });
    },
  },
});
