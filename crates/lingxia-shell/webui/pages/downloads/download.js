(function () {
  var SVG_PAUSE = '<svg viewBox="0 0 16 16" fill="currentColor"><rect x="4" y="3" width="3" height="10" rx="0.5"/><rect x="9" y="3" width="3" height="10" rx="0.5"/></svg>';
  var SVG_FOLDER = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M2 4.5C2 3.67 2.67 3 3.5 3H6l1.5 1.5H12.5c.83 0 1.5.67 1.5 1.5v6c0 .83-.67 1.5-1.5 1.5h-9C2.67 13.5 2 12.83 2 12V4.5z"/></svg>';
  var SVG_REMOVE = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><line x1="4.5" y1="4.5" x2="11.5" y2="11.5"/><line x1="11.5" y1="4.5" x2="4.5" y2="11.5"/></svg>';

  var state = { downloadDir: '', downloads: [], loading: true, error: '' };
  var speedTracker = {};
  var visualProgress = {};
  var downloadsResource = null;
  var unsubscribeDownloads = null;
  var renderScheduled = false;
  var progressAnimationFrame = 0;
  var lastProgressAnimationTs = 0;

  var api = function () { return window.host.downloads; };

  function sortDownloads(arr) {
    function weight(d) {
      if (d.status === 'downloading') return 0;
      if (isPaused(d)) return 1;
      return 2;
    }
    return arr.slice().sort(function (a, b) {
      var w = weight(a) - weight(b);
      if (w !== 0) return w;
      return (b.updatedAt || 0) - (a.updatedAt || 0) || (b.createdAt || 0) - (a.createdAt || 0);
    });
  }

  function reduceDownloadsSnapshot(snapshot, event) {
    if (event.kind === 'started' || event.kind === 'progress' || event.kind === 'paused' || event.kind === 'completed' || event.kind === 'failed') {
      var byId = {};
      (snapshot.downloads || []).forEach(function (download) { byId[download.taskId] = download; });
      byId[event.download.taskId] = event.download;
      var downloads = sortDownloads(Object.keys(byId).map(function (taskId) { return byId[taskId]; }));
      return {
        downloadDir: snapshot.downloadDir || '',
        downloads: downloads,
        hasActiveDownloads: downloads.some(function (download) { return download.status === 'downloading'; }),
      };
    }

    if (event.kind === 'removed') {
      var remaining = (snapshot.downloads || []).filter(function (download) { return download.taskId !== event.taskId; });
      return {
        downloadDir: snapshot.downloadDir || '',
        downloads: remaining,
        hasActiveDownloads: remaining.some(function (download) { return download.status === 'downloading'; }),
      };
    }

    var cleared = (snapshot.downloads || []).filter(function (download) { return download.status !== 'completed'; });
    return {
      downloadDir: snapshot.downloadDir || '',
      downloads: cleared,
      hasActiveDownloads: cleared.some(function (download) { return download.status === 'downloading'; }),
    };
  }

  function createDownloadsResource(downloadsApi) {
    var currentState = null;
    var disposed = false;
    var streamHandle = null;
    var reloadPromise = null;
    var bufferedEvents = null;
    var listeners = new Set();

    function emit() {
      if (!currentState) return;
      listeners.forEach(function (listener) {
        try { listener(currentState); } catch (err) { console.warn('[DL] resource listener failed', err); }
      });
    }

    function reloadInBackground() {
      load().catch(function (err) {
        if (!disposed) console.warn('[DL] resource reload failed', err);
      });
    }

    function attachStream(pendingEvents) {
      var previousHandle = streamHandle;
      var nextHandle = downloadsApi.watch();
      bufferedEvents = pendingEvents || null;
      streamHandle = nextHandle;
      if (previousHandle && typeof previousHandle.cancel === 'function') previousHandle.cancel();

      nextHandle.on('data', function (event) {
        if (disposed || streamHandle !== nextHandle) return;
        if (bufferedEvents) {
          bufferedEvents.push(event);
          return;
        }
        if (!currentState) return;
        currentState = reduceDownloadsSnapshot(currentState, event);
        emit();
      });
      nextHandle.on('error', function (err) {
        if (disposed || streamHandle !== nextHandle) return;
        console.warn('[DL] resource stream failed', err);
        reloadInBackground();
      });
      nextHandle.on('end', function () {
        if (disposed || streamHandle !== nextHandle) return;
        console.warn('[DL] resource stream ended; reloading');
        reloadInBackground();
      });
      if (nextHandle.result && typeof nextHandle.result.catch === 'function') {
        nextHandle.result.catch(function () {});
      }
      return nextHandle;
    }

    function load() {
      if (reloadPromise) return reloadPromise;

      reloadPromise = Promise.resolve().then(function () {
        var pendingEvents = [];
        var activeHandle = attachStream(pendingEvents);
        return Promise.resolve(downloadsApi.list()).then(function (snapshot) {
          var nextState = {
            downloadDir: (snapshot && snapshot.downloadDir) || '',
            downloads: sortDownloads((snapshot && snapshot.downloads) || []),
            hasActiveDownloads: !!(snapshot && snapshot.hasActiveDownloads),
          };
          if (disposed) return nextState;
          if (streamHandle !== activeHandle) return currentState || nextState;
          pendingEvents.forEach(function (event) {
            nextState = reduceDownloadsSnapshot(nextState, event);
          });
          bufferedEvents = null;
          currentState = nextState;
          emit();
          return nextState;
        }).catch(function (err) {
          if (streamHandle === activeHandle) {
            streamHandle = null;
            if (activeHandle && typeof activeHandle.cancel === 'function') activeHandle.cancel();
          }
          throw err;
        });
      }).finally(function () {
        reloadPromise = null;
      });

      return reloadPromise;
    }

    return {
      load: load,
      getState: function () { return currentState; },
      subscribe: function (listener) {
        listeners.add(listener);
        if (currentState) {
          try { listener(currentState); } catch (err) { console.warn('[DL] resource listener failed', err); }
        }
        return function () { listeners.delete(listener); };
      },
      dispose: function () {
        disposed = true;
        var activeHandle = streamHandle;
        streamHandle = null;
        if (activeHandle && typeof activeHandle.cancel === 'function') activeHandle.cancel();
        listeners.clear();
      },
    };
  }

  function fileIcon(name) {
    var ext = String(name || '').split('.').pop().toLowerCase();
    if (['png','jpg','jpeg','gif','webp','svg','ico','bmp','tiff'].indexOf(ext) >= 0) return '🖼️';
    if (['mp4','mov','mkv','avi','webm','flv','m4v'].indexOf(ext) >= 0) return '🎬';
    if (['mp3','wav','flac','aac','ogg','m4a','wma'].indexOf(ext) >= 0) return '🎵';
    if (['zip','rar','7z','tar','gz','bz2','xz','zst','dmg'].indexOf(ext) >= 0) return '🗜️';
    if (ext === 'pdf') return '📄';
    if (['doc','docx','txt','md','rtf','pages'].indexOf(ext) >= 0) return '📝';
    if (['xls','xlsx','csv','numbers'].indexOf(ext) >= 0) return '📊';
    if (['ppt','pptx','key'].indexOf(ext) >= 0) return '📊';
    if (['js','ts','rs','py','go','java','c','cpp','h','swift','kt','html','css','json','xml','yaml','yml','toml'].indexOf(ext) >= 0) return '💻';
    if (['exe','msi','app','deb','rpm','apk','ipa'].indexOf(ext) >= 0) return '⚙️';
    return '📁';
  }

  function fmtBytes(b) {
    if (b == null || isNaN(b)) return '';
    b = Number(b);
    if (b < 1024) return b + ' B';
    var u = ['KB','MB','GB','TB'], s = b / 1024, i = 0;
    while (s >= 1024 && i < u.length - 1) { s /= 1024; i++; }
    return (s >= 100 ? s.toFixed(0) : s >= 10 ? s.toFixed(1) : s.toFixed(2)) + ' ' + u[i];
  }

  function fmtSpeed(bytesPerSec) {
    if (!bytesPerSec || bytesPerSec <= 0) return '';
    return fmtBytes(bytesPerSec) + '/s';
  }

  function fmtDate(ms) {
    if (!ms) return '';
    try { return new Date(ms).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }); } catch (_) { return ''; }
  }

  function dateGroup(ms) {
    if (!ms) return 'Earlier';
    var d = new Date(ms), now = new Date();
    var today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    var yesterday = new Date(today); yesterday.setDate(yesterday.getDate() - 1);
    if (d >= today) return 'Today';
    if (d >= yesterday) return 'Yesterday';
    return d.toLocaleDateString(undefined, { month: 'long', day: 'numeric', year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined });
  }

  function pct(item) {
    var t = Number(item.totalBytes || 0);
    return t > 0 ? Math.min(100, (Number(item.downloadedBytes || 0) / t) * 100) : null;
  }

  function animationNow() {
    return (window.performance && typeof window.performance.now === 'function')
      ? window.performance.now()
      : Date.now();
  }

  function updateVisualProgressTargets(downloads) {
    var now = animationNow();
    var keep = {};
    downloads.forEach(function (item) {
      keep[item.taskId] = 1;
      var raw = pct(item);
      if (raw == null) {
        delete visualProgress[item.taskId];
        return;
      }
      var entry = visualProgress[item.taskId];
      if (!entry) {
        entry = visualProgress[item.taskId] = { value: raw, target: raw, updatedAt: now };
      }
      entry.updatedAt = now;
      if (item.status === 'completed') {
        entry.value = 100;
        entry.target = 100;
        return;
      }
      if (item.status === 'paused') {
        entry.value = raw;
        entry.target = raw;
        return;
      }
      if (item.status !== 'downloading') {
        entry.value = raw;
        entry.target = raw;
        return;
      }
      entry.target = raw;
      if (entry.value > raw) entry.value = raw;
    });
    Object.keys(visualProgress).forEach(function (taskId) {
      if (!keep[taskId]) delete visualProgress[taskId];
    });
  }

  function hasAnimatingProgress() {
    return state.downloads.some(function (item) {
      if (item.status !== 'downloading') return false;
      var raw = pct(item);
      if (raw == null) return false;
      var entry = visualProgress[item.taskId];
      return !!entry && raw - entry.value > 0.02;
    });
  }

  function ensureProgressAnimation() {
    if (progressAnimationFrame || !hasAnimatingProgress()) return;
    progressAnimationFrame = requestAnimationFrame(stepProgressAnimation);
  }

  function stepProgressAnimation(ts) {
    progressAnimationFrame = 0;
    var dt = lastProgressAnimationTs ? Math.min(64, ts - lastProgressAnimationTs) : 16;
    lastProgressAnimationTs = ts;
    var changed = false;
    state.downloads.forEach(function (item) {
      if (item.status !== 'downloading') return;
      var raw = pct(item);
      if (raw == null) return;
      var entry = visualProgress[item.taskId];
      if (!entry) {
        visualProgress[item.taskId] = { value: raw, target: raw, updatedAt: ts };
        return;
      }
      entry.target = raw;
      entry.updatedAt = ts;
      if (entry.value >= entry.target) {
        entry.value = entry.target;
        return;
      }
      var next = entry.value + (entry.target - entry.value) * Math.min(1, dt / 180);
      if (entry.target - next < 0.02) next = entry.target;
      if (Math.abs(next - entry.value) > 0.001) {
        entry.value = next;
        changed = true;
      }
    });
    if (changed) scheduleRender();
    if (hasAnimatingProgress()) {
      progressAnimationFrame = requestAnimationFrame(stepProgressAnimation);
    } else {
      lastProgressAnimationTs = 0;
    }
  }

  function displayedPct(item) {
    var raw = pct(item);
    if (raw == null) return null;
    var entry = visualProgress[item.taskId];
    if (!entry) return raw;
    if (item.status === 'completed') return 100;
    if (item.status === 'paused') return entry.target;
    return Math.max(0, Math.min(100, entry.value));
  }

  function formatPct(value) {
    if (value == null || !isFinite(value)) return '';
    return value >= 99.95 ? '100%' : value.toFixed(1).replace(/\.0$/, '') + '%';
  }

  function isPaused(item) {
    return item.status === 'paused';
  }

  function trackSpeed(item) {
    var id = item.taskId, now = Date.now();
    var t = speedTracker[id];
    if (!t) { t = speedTracker[id] = { bytes: item.downloadedBytes, time: now, speed: 0 }; return 0; }
    var dt = now - t.time;
    if (dt > 400) {
      var db = item.downloadedBytes - t.bytes;
      t.speed = db > 0 ? (db / dt) * 1000 : t.speed * 0.5;
      t.bytes = item.downloadedBytes;
      t.time = now;
    }
    return t.speed;
  }

  function urlHost(url) {
    try { return new URL(url).host; } catch (_) { return url; }
  }

  function escHtml(s) {
    return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
  }

  function mkBtn(cls, innerHTML, title) {
    var b = document.createElement('button');
    b.type = 'button'; b.className = cls; b.title = title;
    b.setAttribute('aria-label', title);
    b.innerHTML = innerHTML;
    b.hidden = true;
    return b;
  }

  function createItem() {
    var el = document.createElement('div');
    el.className = 'item';

    var icon = document.createElement('div');
    icon.className = 'file-icon';

    var body = document.createElement('div');
    body.className = 'item-body';

    var row1 = document.createElement('div');
    row1.className = 'item-row';
    var nameLink = document.createElement('button');
    nameLink.type = 'button';
    nameLink.className = 'file-name file-name-link';
    nameLink.hidden = true;
    var nameSpan = document.createElement('span');
    nameSpan.className = 'file-name';
    var controls = document.createElement('div');
    controls.className = 'item-controls';

    var pauseBtn = mkBtn('ctrl-btn', SVG_PAUSE, 'Pause');
    var revealBtn = mkBtn('ctrl-btn', SVG_FOLDER, 'Show in Finder');
    var removeBtn = mkBtn('ctrl-btn danger', SVG_REMOVE, 'Remove');
    controls.appendChild(pauseBtn);
    controls.appendChild(revealBtn);
    controls.appendChild(removeBtn);

    row1.appendChild(nameLink);
    row1.appendChild(nameSpan);
    row1.appendChild(controls);

    var detail = document.createElement('div');
    detail.className = 'item-detail';

    var progressRow = document.createElement('div');
    progressRow.className = 'progress-row'; progressRow.hidden = true;
    var track = document.createElement('div'); track.className = 'progress-track';
    var bar = document.createElement('div'); bar.className = 'progress-bar';
    track.appendChild(bar);
    var pInfo = document.createElement('span'); pInfo.className = 'progress-info';
    progressRow.appendChild(track);
    progressRow.appendChild(pInfo);

    var actionRow = document.createElement('div');
    actionRow.className = 'action-row'; actionRow.hidden = true;
    var resumeBtn = document.createElement('button');
    resumeBtn.type = 'button'; resumeBtn.className = 'act-btn'; resumeBtn.textContent = 'Resume'; resumeBtn.hidden = true;
    var retryBtn = document.createElement('button');
    retryBtn.type = 'button'; retryBtn.className = 'act-btn'; retryBtn.textContent = 'Retry'; retryBtn.hidden = true;
    var cancelBtn = document.createElement('button');
    cancelBtn.type = 'button'; cancelBtn.className = 'act-btn warn'; cancelBtn.textContent = 'Cancel'; cancelBtn.hidden = true;
    var cancelDlBtn = document.createElement('button');
    cancelDlBtn.type = 'button'; cancelDlBtn.className = 'act-btn warn'; cancelDlBtn.textContent = 'Cancel download'; cancelDlBtn.hidden = true;
    actionRow.appendChild(resumeBtn);
    actionRow.appendChild(retryBtn);
    actionRow.appendChild(cancelBtn);
    actionRow.appendChild(cancelDlBtn);

    body.appendChild(row1);
    body.appendChild(detail);
    body.appendChild(progressRow);
    body.appendChild(actionRow);
    el.appendChild(icon);
    el.appendChild(body);

    var r = {
      icon: icon, nameLink: nameLink, nameSpan: nameSpan,
      detail: detail, bar: bar, pInfo: pInfo,
      progressRow: progressRow, actionRow: actionRow,
      pauseBtn: pauseBtn, revealBtn: revealBtn, removeBtn: removeBtn,
      resumeBtn: resumeBtn, retryBtn: retryBtn, cancelBtn: cancelBtn, cancelDlBtn: cancelDlBtn,
    };
    el._r = r;

    nameLink.addEventListener('click', function (e) {
      var tid = el.dataset.taskId;
      console.log('[DL] open', tid);
      api().open({ taskId: tid }).then(function () { console.log('[DL] open OK', tid); }, function (err) { console.error('[DL] open FAIL', tid, err); });
    });
    pauseBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] pause', tid);
      api().pause({ taskId: tid }).then(function () { console.log('[DL] pause OK', tid); }, function (err) { console.error('[DL] pause FAIL', tid, err); });
    });
    revealBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] reveal', tid);
      api().reveal({ taskId: tid }).then(function () { console.log('[DL] reveal OK', tid); }, function (err) { console.error('[DL] reveal FAIL', tid, err); });
    });
    removeBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] remove', tid);
      api().remove({ taskId: tid }).then(function () { console.log('[DL] remove OK', tid); }, function (err) { console.error('[DL] remove FAIL', tid, err); });
      state.downloads = state.downloads.filter(function (d) { return d.taskId !== tid; });
      scheduleRender();
    });
    resumeBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] resume', tid);
      api().resume({ taskId: tid }).then(function () { console.log('[DL] resume OK', tid); }, function (err) { console.error('[DL] resume FAIL', tid, err); });
    });
    retryBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] retry', tid);
      api().retry({ taskId: tid }).then(function () { console.log('[DL] retry OK', tid); }, function (err) { console.error('[DL] retry FAIL', tid, err); });
    });
    cancelBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] cancelRemove', tid);
      api().remove({ taskId: tid }).then(function () { console.log('[DL] cancelRemove OK', tid); }, function (err) { console.error('[DL] cancelRemove FAIL', tid, err); });
      state.downloads = state.downloads.filter(function (d) { return d.taskId !== tid; });
      scheduleRender();
    });
    cancelDlBtn.addEventListener('click', function () {
      var tid = el.dataset.taskId;
      console.log('[DL] cancelDl', tid);
      api().cancel({ taskId: tid }).then(function () { console.log('[DL] cancelDl OK', tid); }, function (err) { console.error('[DL] cancelDl FAIL', tid, err); });
    });

    return el;
  }

  function updateItem(el, item) {
    var r = el._r;
    el.dataset.taskId = item.taskId;

    var isActive = item.status === 'downloading';
    var isDone = item.status === 'completed';
    var isFailed = item.status === 'failed';
    var paused = isPaused(item);

    r.icon.textContent = fileIcon(item.fileName);
    r.icon.className = 'file-icon' + (isActive ? ' is-active' : '') + (isFailed ? ' is-failed' : '');

    r.nameLink.hidden = !isDone;
    r.nameSpan.hidden = isDone;
    if (isDone) {
      r.nameLink.textContent = item.fileName;
      r.nameLink.title = 'Open ' + item.fileName;
    }
    else {
      r.nameSpan.textContent = item.fileName;
      r.nameSpan.title = item.fileName;
      r.nameLink.title = '';
    }

    var parts = [];
    if (isDone) {
      parts.push(fmtBytes(item.totalBytes != null ? item.totalBytes : item.downloadedBytes));
      parts.push(fmtDate(item.completedAt || item.updatedAt));
      parts.push(escHtml(urlHost(item.url)));
    } else if (isActive) {
      var dl = fmtBytes(item.downloadedBytes);
      var tot = item.totalBytes ? fmtBytes(item.totalBytes) : null;
      parts.push(tot ? dl + ' / ' + tot : dl);
      var spd = fmtSpeed(trackSpeed(item));
      if (spd) parts.push(spd);
      parts.push(escHtml(urlHost(item.url)));
    } else if (paused) {
      var ps = fmtBytes(item.downloadedBytes);
      if (item.totalBytes) ps += ' / ' + fmtBytes(item.totalBytes);
      parts.push(ps);
      parts.push('Paused');
    } else if (isFailed) {
      parts.push('<span class="error-text">' + escHtml(item.error || 'Download failed') + '</span>');
    } else {
      parts.push('Removed');
    }
    r.detail.innerHTML = parts.join('<span class="sep"> · </span>');

    r.progressRow.hidden = !isActive && !paused;
    if (isActive || paused) {
      var p = displayedPct(item);
      r.bar.className = 'progress-bar' +
        (paused ? ' paused' : '') +
        (isActive && p == null ? ' indeterminate' : '');
      r.bar.style.transform = 'scaleX(' + (p != null ? (p / 100).toFixed(4) : 0.34) + ')';
      r.pInfo.className = 'progress-info' + (paused ? ' paused' : '');
      r.pInfo.textContent = p != null ? formatPct(p) : fmtBytes(item.downloadedBytes);
    }

    r.pauseBtn.hidden = !isActive;
    r.revealBtn.hidden = !isDone;
    r.removeBtn.hidden = isActive;

    r.resumeBtn.hidden = !paused;
    r.retryBtn.hidden = !(isFailed && !paused);
    r.cancelBtn.hidden = !paused;
    r.cancelDlBtn.hidden = !isActive;
    r.actionRow.hidden = r.resumeBtn.hidden && r.retryBtn.hidden && r.cancelBtn.hidden && r.cancelDlBtn.hidden;
  }

  var lastRenderKey = '';

  function buildGroups() {
    var groups = [], lastGroup = '';
    state.downloads.forEach(function (item) {
      var g = item.status === 'downloading' ? 'Downloading' : isPaused(item) ? 'Paused' : dateGroup(item.completedAt || item.updatedAt || item.createdAt);
      if (g !== lastGroup) { groups.push({ label: g, items: [] }); lastGroup = g; }
      groups[groups.length - 1].items.push(item);
    });
    return groups;
  }

  function structureKey(groups) {
    var parts = [];
    groups.forEach(function (g) {
      parts.push(g.label + ':');
      g.items.forEach(function (item) { parts.push(item.taskId + '/' + item.status); });
    });
    return parts.join('|');
  }

  function render() {
    var list = document.getElementById('list');
    var clearBtn = document.getElementById('clearBtn');
    var statusEl = document.getElementById('statusText');
    var errorEl = document.getElementById('errorBanner');

    if (state.error) {
      errorEl.textContent = state.error; errorEl.style.display = '';
    } else {
      errorEl.textContent = ''; errorEl.style.display = 'none';
    }

    var active = state.downloads.filter(function (d) { return d.status === 'downloading'; }).length;
    statusEl.textContent = state.loading ? 'Loading...'
      : active > 0 ? active + ' downloading'
      : state.downloads.length === 0 ? '' : state.downloads.length + ' file' + (state.downloads.length > 1 ? 's' : '');

    clearBtn.style.display = state.downloads.some(function (d) { return d.status !== 'downloading'; }) ? '' : 'none';

    if (state.loading) {
      list.innerHTML = '<div class="empty"><span class="empty-icon">⏳</span><p>Loading...</p></div>';
      lastRenderKey = '';
      return;
    }
    if (state.downloads.length === 0) {
      list.innerHTML = '<div class="empty"><span class="empty-icon">📥</span><p>No downloads yet</p><span>Files you download will appear here</span></div>';
      lastRenderKey = '';
      return;
    }

    var groups = buildGroups();
    var key = structureKey(groups);

    if (key === lastRenderKey) {
      var itemMap = {};
      state.downloads.forEach(function (d) { itemMap[d.taskId] = d; });
      list.querySelectorAll('.item[data-task-id]').forEach(function (el) {
        var item = itemMap[el.dataset.taskId];
        if (item) updateItem(el, item);
      });
      cleanSpeedTracker();
      return;
    }

    var existingMap = {};
    list.querySelectorAll('.item[data-task-id]').forEach(function (n) {
      existingMap[n.dataset.taskId] = n;
    });

    var frag = document.createDocumentFragment();
    groups.forEach(function (g) {
      var section = document.createElement('div');
      section.className = 'date-group';
      var label = document.createElement('div');
      label.className = 'date-label'; label.textContent = g.label;
      section.appendChild(label);

      g.items.forEach(function (item) {
        var el = existingMap[item.taskId];
        if (el) { delete existingMap[item.taskId]; } else { el = createItem(); }
        updateItem(el, item);
        section.appendChild(el);
      });
      frag.appendChild(section);
    });

    list.innerHTML = '';
    list.appendChild(frag);
    lastRenderKey = key;

    cleanSpeedTracker();
  }

  function cleanSpeedTracker() {
    var activeIds = {};
    state.downloads.forEach(function (d) { if (d.status === 'downloading') activeIds[d.taskId] = 1; });
    Object.keys(speedTracker).forEach(function (k) { if (!activeIds[k]) delete speedTracker[k]; });
    Object.keys(visualProgress).forEach(function (k) { if (!activeIds[k] && !state.downloads.some(function (d) { return d.taskId === k; })) delete visualProgress[k]; });
  }

  function scheduleRender() {
    if (renderScheduled) return;
    renderScheduled = true;
    requestAnimationFrame(function () { renderScheduled = false; render(); });
  }

  function applySnapshot(snap) {
    console.log('[DL] applySnapshot', snap && snap.downloads ? snap.downloads.length + ' items' : 'empty', snap);
    state.downloadDir = (snap && snap.downloadDir) || '';
    state.downloads = sortDownloads((snap && snap.downloads) || []);
    updateVisualProgressTargets(state.downloads);
    state.loading = false;
    state.error = '';
    ensureProgressAnimation();
  }

  document.getElementById('clearBtn').addEventListener('click', function () {
    api().clearCompleted();
    state.downloads = state.downloads.filter(function (d) { return d.status === 'downloading'; });
    scheduleRender();
  });

  window.addEventListener('pagehide', function () {
    if (progressAnimationFrame) {
      cancelAnimationFrame(progressAnimationFrame);
      progressAnimationFrame = 0;
      lastProgressAnimationTs = 0;
    }
    if (unsubscribeDownloads) { unsubscribeDownloads(); unsubscribeDownloads = null; }
    if (downloadsResource) { downloadsResource.dispose(); downloadsResource = null; }
  });

  console.log('[DL] creating local resource');
  downloadsResource = createDownloadsResource(api());
  unsubscribeDownloads = downloadsResource.subscribe(function (snap) {
    applySnapshot(snap); scheduleRender();
  });
  downloadsResource.load().then(function (snap) {
    console.log('[DL] initial load OK');
    applySnapshot(snap); render();
  }).catch(function (err) {
    console.error('[DL] initial load FAIL', err);
    state.loading = false;
    state.error = err instanceof Error ? err.message : 'Failed to load';
    render();
  });

  render();
})();
