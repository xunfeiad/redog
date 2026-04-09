// Tauri v2 API - robust detection
let invoke, listen;
try {
  const tauri = window.__TAURI__;
  if (tauri) {
    invoke = (tauri.core && tauri.core.invoke) || (tauri.tauri && tauri.tauri.invoke);
    listen = tauri.event && tauri.event.listen;
  }
} catch (e) {
  console.error('Failed to initialize Tauri API:', e);
}

if (!invoke) {
  console.warn('Tauri invoke API not found, GUI commands will not work');
}

// Wrapper to safely call invoke with error handling
async function safeInvoke(cmd, args) {
  if (!invoke) throw new Error('Tauri API 未就绪');
  return invoke(cmd, args || {});
}

// --- Node & Subscription storage ---
let subscriptions = [];
let manualNodes = [];
const STORAGE_KEY_SUBS = 'redog_subscriptions';
const STORAGE_KEY_NODES = 'redog_manual_nodes';

function loadLocalStorage() {
  try {
    subscriptions = JSON.parse(localStorage.getItem(STORAGE_KEY_SUBS) || '[]');
    manualNodes = JSON.parse(localStorage.getItem(STORAGE_KEY_NODES) || '[]');
  } catch (e) {
    subscriptions = [];
    manualNodes = [];
  }
}
function saveLocalStorage() {
  localStorage.setItem(STORAGE_KEY_SUBS, JSON.stringify(subscriptions));
  localStorage.setItem(STORAGE_KEY_NODES, JSON.stringify(manualNodes));
}
loadLocalStorage();

// --- Connection status indicator ---
let apiConnected = false;

function setApiStatus(connected) {
  apiConnected = connected;
  const el = document.getElementById('api-status');
  if (el) {
    el.className = 'api-status ' + (connected ? 'online' : 'offline');
    el.textContent = connected ? 'API 已连接' : 'API 未连接';
  }
}

// --- View navigation ---
document.querySelectorAll('.nav-item').forEach(btn => {
  btn.addEventListener('click', () => navigate(btn.dataset.view));
});

function navigate(view) {
  document.querySelectorAll('.nav-item').forEach(b =>
    b.classList.toggle('active', b.dataset.view === view)
  );
  document.querySelectorAll('.view').forEach(v =>
    v.classList.toggle('active', v.id === 'view-' + view)
  );
  refreshView(view);
}

if (listen) {
  listen('navigate', (e) => navigate(e.payload));
  listen('latency-test', async () => await testAllLatency());
}

// --- Global latency test button ---
var btnTestAll = document.getElementById('btn-test-all');
if (btnTestAll) {
  btnTestAll.addEventListener('click', function() { testAllLatency(); });
}

// --- Mode tabs ---
document.querySelectorAll('.mode-tab').forEach(tab => {
  tab.addEventListener('click', async () => {
    document.querySelectorAll('.mode-tab').forEach(t => t.classList.remove('active'));
    tab.classList.add('active');
    try {
      await safeInvoke('patch_configs', { body: { mode: tab.dataset.mode } });
    } catch (e) { console.warn('patch mode:', e); }
  });
});

// --- Proxies view ---
async function loadProxies() {
  const container = document.getElementById('groups-list');
  try {
    const data = await safeInvoke('get_proxies');
    setApiStatus(true);
    renderProxies(data.proxies || data || {});
  } catch (e) {
    setApiStatus(false);
    container.innerHTML =
      '<div class="group-card"><div class="group-header"><div class="group-title">无法连接到 API 后端</div></div>'
      + '<div style="color:var(--muted);font-size:12px">' + escapeHtml(String(e)) + '</div>'
      + '<div style="color:var(--muted);font-size:12px;margin-top:8px">请确保 Redog 内核或 Clash 已启动且 external-controller 为 127.0.0.1:9090</div></div>';
  }
}

function renderProxies(proxies) {
  const container = document.getElementById('groups-list');
  const groups = Object.values(proxies).filter(p => p.all && p.all.length > 0);
  if (groups.length === 0) {
    container.innerHTML = '<div class="group-card"><div class="group-title">无代理组</div></div>';
    return;
  }
  container.innerHTML = groups.map(g => {
    const chips = g.all.map(name => {
      const p = proxies[name] || {};
      const delay = (p.history && p.history.length > 0) ? p.history[p.history.length - 1].delay : 0;
      const delayClass = delay === 0 ? '' : (delay < 200 ? 'good' : (delay < 500 ? 'ok' : 'bad'));
      const delayText = delay === 0 ? '-' : delay + 'ms';
      const selected = g.now === name ? 'selected' : '';
      return '<div class="proxy-chip ' + selected + '" data-name="' + escapeHtml(name) + '">'
        + '<span class="name">' + escapeHtml(name) + '</span>'
        + '<span class="delay ' + delayClass + '">' + delayText + '</span>'
        + '</div>';
    }).join('');
    return '<div class="group-card" data-group="' + escapeHtml(g.name) + '">'
      + '<div class="group-header">'
      + '<div class="group-title">' + escapeHtml(g.name) + '</div>'
      + '<div style="display:flex;align-items:center;gap:6px">'
      + '<button class="btn btn-sm btn-test-group" title="测试该组延迟">测速</button>'
      + '<div class="group-type">' + escapeHtml(g.type || '') + '</div>'
      + '</div>'
      + '</div>'
      + '<div class="proxy-grid">' + chips + '</div>'
      + '</div>';
  }).join('');

  container.querySelectorAll('.group-card').forEach(card => {
    const groupName = card.dataset.group;
    // Click to select proxy
    card.querySelectorAll('.proxy-chip').forEach(chip => {
      chip.addEventListener('click', async () => {
        var nodeName = chip.dataset.name;
        console.log('[select_proxy] group=' + groupName + ', name=' + nodeName);
        chip.style.opacity = '0.5';
        try {
          var res = await safeInvoke('select_proxy', { group: groupName, name: nodeName });
          console.log('[select_proxy] result:', res);
          await loadProxies();
        } catch (e) {
          console.error('[select_proxy] error:', e);
          chip.style.opacity = '1';
        }
      });
    });
    // Per-group latency test button
    var groupTestBtn = card.querySelector('.btn-test-group');
    if (groupTestBtn) {
      groupTestBtn.addEventListener('click', async function(e) {
        e.stopPropagation();
        groupTestBtn.textContent = '测速中...';
        groupTestBtn.disabled = true;
        await testGroupLatency(card);
        groupTestBtn.textContent = '测速';
        groupTestBtn.disabled = false;
      });
    }
  });
}

// Test latency for all chips in a specific container
async function testGroupLatency(container) {
  var chips = container.querySelectorAll('.proxy-chip');
  var promises = [];
  for (var i = 0; i < chips.length; i++) {
    (function(chip) {
      var name = chip.dataset.name;
      var span = chip.querySelector('.delay');
      if (span) span.textContent = '...';
      promises.push(
        safeInvoke('test_latency', { name: name }).then(function(res) {
          var delay = (res && res.delay) || 0;
          if (span) {
            if (delay > 0) {
              span.textContent = delay + 'ms';
              span.className = 'delay ' + (delay < 200 ? 'good' : (delay < 500 ? 'ok' : 'bad'));
            } else {
              span.textContent = '超时';
              span.className = 'delay bad';
            }
          }
        }).catch(function() {
          if (span) {
            span.textContent = '失败';
            span.className = 'delay bad';
          }
        })
      );
    })(chips[i]);
  }
  await Promise.all(promises);
}

// Test all groups
async function testAllLatency() {
  var btn = document.getElementById('btn-test-all');
  if (btn) { btn.textContent = '测速中...'; btn.disabled = true; }
  var cards = document.querySelectorAll('.group-card');
  for (var i = 0; i < cards.length; i++) {
    await testGroupLatency(cards[i]);
  }
  if (btn) { btn.textContent = '测速'; btn.disabled = false; }
}

// =============================================
// --- Nodes view ---
// =============================================
function loadNodes() {
  renderSubscriptions();
  renderNodeList();
}

function renderSubscriptions() {
  const container = document.getElementById('sub-list');
  if (!container) return;
  if (subscriptions.length === 0) {
    container.innerHTML = '<div class="empty-hint">暂无订阅，点击"添加订阅"导入</div>';
    return;
  }
  container.innerHTML = subscriptions.map(function(s, i) {
    return '<div class="sub-row">'
      + '<div class="sub-info">'
      + '<div class="sub-name">' + escapeHtml(s.name) + '</div>'
      + '<div class="sub-detail">' + escapeHtml(truncateUrl(s.url)) + ' | ' + (s.nodes || []).length + ' 个节点 | 更新间隔: ' + (s.interval || 24) + 'h'
      + (s.updatedAt ? ' | 上次: ' + escapeHtml(s.updatedAt) : '') + '</div>'
      + '</div>'
      + '<div class="sub-actions">'
      + '<button class="btn btn-sm" data-sub-update="' + i + '">更新</button>'
      + '<button class="btn btn-sm btn-danger" data-sub-delete="' + i + '">删除</button>'
      + '</div>'
      + '</div>';
  }).join('');

  // Bind events AFTER rendering
  container.querySelectorAll('[data-sub-update]').forEach(function(btn) {
    btn.addEventListener('click', function() {
      var idx = parseInt(btn.dataset.subUpdate);
      btn.textContent = '更新中...';
      btn.disabled = true;
      updateSubscription(idx).finally(function() {
        btn.textContent = '更新';
        btn.disabled = false;
      });
    });
  });
  container.querySelectorAll('[data-sub-delete]').forEach(function(btn) {
    btn.addEventListener('click', function() {
      deleteSubscription(parseInt(btn.dataset.subDelete));
    });
  });
}

function renderNodeList() {
  var allNodes = [];
  // Gather subscription nodes
  subscriptions.forEach(function(s) {
    (s.nodes || []).forEach(function(n) {
      allNodes.push({ name: n.name, type: n.type, server: n.server, port: n.port, source: s.name, id: n.id, fromSub: true });
    });
  });
  // Gather manual nodes
  manualNodes.forEach(function(n) {
    allNodes.push({ name: n.name, type: n.type, server: n.server, port: n.port, source: '手动添加', id: n.id, fromSub: false });
  });

  var countEl = document.getElementById('node-count');
  if (countEl) countEl.textContent = allNodes.length;

  var container = document.getElementById('node-list');
  if (!container) return;

  if (allNodes.length === 0) {
    container.innerHTML = '<div class="empty-hint">暂无节点</div>';
    return;
  }

  container.innerHTML = allNodes.map(function(n) {
    var typeName = (n.type || 'ss').toLowerCase();
    var tagClass = 'tag-' + typeName;
    var deleteBtn = n.fromSub ? '' : '<button class="btn btn-sm btn-danger" data-manual-delete="' + escapeHtml(n.id) + '">删除</button>';
    return '<div class="node-row">'
      + '<div class="node-info">'
      + '<div class="node-name"><span class="node-tag ' + tagClass + '">' + escapeHtml(typeName.toUpperCase()) + '</span>' + escapeHtml(n.name) + '</div>'
      + '<div class="node-detail">' + escapeHtml(n.server || '') + ':' + (n.port || '') + ' | 来源: ' + escapeHtml(n.source) + '</div>'
      + '</div>'
      + '<div class="node-actions">' + deleteBtn + '</div>'
      + '</div>';
  }).join('');

  // Bind delete events
  container.querySelectorAll('[data-manual-delete]').forEach(function(btn) {
    btn.addEventListener('click', function() {
      var id = btn.dataset.manualDelete;
      manualNodes = manualNodes.filter(function(n) { return n.id !== id; });
      saveLocalStorage();
      renderNodeList();
    });
  });
}

// --- Subscription modal ---
var btnAddSub = document.getElementById('btn-add-sub');
if (btnAddSub) {
  btnAddSub.addEventListener('click', function() {
    document.getElementById('sub-name').value = '';
    document.getElementById('sub-url').value = '';
    document.getElementById('sub-interval').value = '24';
    document.getElementById('modal-sub').style.display = 'flex';
  });
}

var btnSubCancel = document.getElementById('btn-sub-cancel');
if (btnSubCancel) {
  btnSubCancel.addEventListener('click', function() {
    document.getElementById('modal-sub').style.display = 'none';
  });
}

var btnSubSave = document.getElementById('btn-sub-save');
if (btnSubSave) {
  btnSubSave.addEventListener('click', async function() {
    var name = document.getElementById('sub-name').value.trim();
    var url = document.getElementById('sub-url').value.trim();
    var interval = parseInt(document.getElementById('sub-interval').value) || 24;
    if (!name || !url) { alert('请填写名称和链接'); return; }
    if (!url.startsWith('http://') && !url.startsWith('https://')) {
      alert('订阅链接必须以 http:// 或 https:// 开头'); return;
    }

    var sub = { id: genId(), name: name, url: url, interval: interval, nodes: [], updatedAt: '' };
    subscriptions.push(sub);
    saveLocalStorage();
    document.getElementById('modal-sub').style.display = 'none';
    // Update immediately
    await updateSubscription(subscriptions.length - 1);
  });
}

async function updateSubscription(index) {
  var sub = subscriptions[index];
  if (!sub) return;

  try {
    // Try Tauri backend first (handles CORS)
    var result = await safeInvoke('fetch_subscription', { url: sub.url });
    if (result && result.nodes) {
      sub.nodes = result.nodes.map(function(n) {
        n.id = n.id || genId();
        return n;
      });
      sub.updatedAt = new Date().toLocaleString();
      saveLocalStorage();
      loadNodes();
      return;
    }
  } catch (e) {
    console.warn('Tauri fetch failed, trying direct:', e);
  }

  // Fallback: direct fetch
  try {
    var resp = await fetch(sub.url, { headers: { 'User-Agent': 'Redog/0.1' } });
    if (!resp.ok) throw new Error('HTTP ' + resp.status);
    var text = await resp.text();
    sub.nodes = parseSubscriptionText(text);
    sub.updatedAt = new Date().toLocaleString();
    saveLocalStorage();
    loadNodes();
  } catch (e2) {
    alert('更新订阅失败: ' + e2.message);
  }
}

function deleteSubscription(index) {
  if (!confirm('确定删除订阅 "' + subscriptions[index].name + '"？其下所有节点将被移除。')) return;
  subscriptions.splice(index, 1);
  saveLocalStorage();
  loadNodes();
}

// --- Parse subscription content ---
function parseSubscriptionText(text) {
  var nodes = [];
  var trimmed = text.trim();

  // 1. Try base64 decode (common for ss/vmess subscriptions)
  try {
    var decoded = atob(trimmed.replace(/[\r\n\s]/g, ''));
    var lines = decoded.split('\n').filter(function(l) { return l.trim(); });
    for (var i = 0; i < lines.length; i++) {
      var node = parseShareUri(lines[i].trim());
      if (node) nodes.push(node);
    }
    if (nodes.length > 0) return nodes;
  } catch (e) { /* not base64 */ }

  // 2. Try as Clash YAML format (multi-line support)
  try {
    nodes = parseClashYaml(trimmed);
    if (nodes.length > 0) return nodes;
  } catch (e) { /* not yaml */ }

  // 3. Try line-by-line share URIs
  var lines2 = trimmed.split('\n');
  for (var j = 0; j < lines2.length; j++) {
    var node2 = parseShareUri(lines2[j].trim());
    if (node2) nodes.push(node2);
  }

  return nodes;
}

// Parse Clash YAML config - handles both inline and multi-line format
function parseClashYaml(text) {
  var nodes = [];
  var lines = text.split('\n');
  var inProxies = false;
  var currentNode = null;

  for (var i = 0; i < lines.length; i++) {
    var line = lines[i];
    var trimLine = line.trim();

    // Detect proxies section
    if (/^proxies\s*:/.test(trimLine)) {
      inProxies = true;
      continue;
    }
    // Exit proxies section if new top-level key
    if (inProxies && /^\S/.test(line) && !trimLine.startsWith('-')) {
      inProxies = false;
      if (currentNode && currentNode.name) { nodes.push(currentNode); currentNode = null; }
      continue;
    }

    if (!inProxies) continue;

    // Inline format: - {name: xxx, type: ss, ...}
    var inlineMatch = trimLine.match(/^-\s*\{(.+)\}\s*$/);
    if (inlineMatch) {
      if (currentNode && currentNode.name) nodes.push(currentNode);
      currentNode = null;
      try {
        // Convert YAML inline to JSON-ish
        var jsonStr = '{' + inlineMatch[1] + '}';
        // Handle unquoted values and single quotes
        jsonStr = jsonStr.replace(/:\s*([^,}\s][^,}]*)/g, function(m, v) {
          v = v.trim();
          if (v === 'true' || v === 'false' || /^\d+$/.test(v)) return ': ' + v;
          return ': "' + v.replace(/"/g, '\\"') + '"';
        });
        jsonStr = jsonStr.replace(/(\w[\w-]*)\s*:/g, '"$1":');
        jsonStr = jsonStr.replace(/'/g, '"');
        var obj = JSON.parse(jsonStr);
        nodes.push(yamlObjToNode(obj));
      } catch (e) { /* skip malformed inline */ }
      continue;
    }

    // Multi-line: - name: xxx
    var newItemMatch = trimLine.match(/^-\s+(\w[\w-]*)\s*:\s*(.*)$/);
    if (newItemMatch) {
      if (currentNode && currentNode.name) nodes.push(currentNode);
      currentNode = { id: genId(), name: '', type: 'ss', server: '', port: 0, password: '', cipher: '', tls: false };
      setNodeField(currentNode, newItemMatch[1], newItemMatch[2]);
      continue;
    }

    // Continuation line:   key: value
    if (currentNode) {
      var kvMatch = trimLine.match(/^(\w[\w-]*)\s*:\s*(.*)$/);
      if (kvMatch) {
        setNodeField(currentNode, kvMatch[1], kvMatch[2]);
      }
    }
  }

  // Don't forget the last node
  if (currentNode && currentNode.name) nodes.push(currentNode);

  return nodes;
}

function setNodeField(node, key, val) {
  val = val.replace(/^["']|["']$/g, '').trim(); // strip quotes
  switch (key) {
    case 'name': node.name = val; break;
    case 'type': node.type = val; break;
    case 'server': node.server = val; break;
    case 'port': node.port = parseInt(val) || 0; break;
    case 'password': case 'uuid': node.password = val; break;
    case 'cipher': case 'method': node.cipher = val; break;
    case 'tls': node.tls = (val === 'true'); break;
  }
}

function yamlObjToNode(obj) {
  return {
    id: genId(),
    name: obj.name || 'unnamed',
    type: obj.type || 'ss',
    server: obj.server || '',
    port: parseInt(obj.port) || 0,
    password: obj.password || obj.uuid || '',
    cipher: obj.cipher || obj.method || '',
    tls: obj.tls === true || obj.tls === 'true',
  };
}

// --- Parse share URI ---
function parseShareUri(uri) {
  if (!uri || uri.length < 6) return null;

  // ss://
  if (uri.startsWith('ss://')) {
    try {
      var rest = uri.substring(5);
      var hashIdx = rest.indexOf('#');
      var name = hashIdx >= 0 ? decodeURIComponent(rest.substring(hashIdx + 1)) : 'SS Node';
      var main = hashIdx >= 0 ? rest.substring(0, hashIdx) : rest;
      // Remove query params
      var qIdx = main.indexOf('?');
      if (qIdx >= 0) main = main.substring(0, qIdx);
      // Try base64 decode
      var decoded;
      try { decoded = atob(main); } catch (e) { decoded = main; }
      var atIdx = decoded.lastIndexOf('@');
      if (atIdx < 0) return null;
      var userInfo = decoded.substring(0, atIdx);
      var hostPort = decoded.substring(atIdx + 1);
      var colonIdx = userInfo.indexOf(':');
      var cipher = colonIdx >= 0 ? userInfo.substring(0, colonIdx) : 'aes-256-gcm';
      var password = colonIdx >= 0 ? userInfo.substring(colonIdx + 1) : userInfo;
      var hpParts = hostPort.split(':');
      if (hpParts.length < 2) return null;
      return { id: genId(), name: name, type: 'ss', server: hpParts[0], port: parseInt(hpParts[1]) || 0, password: password, cipher: cipher, tls: false };
    } catch (e) { return null; }
  }

  // vmess://
  if (uri.startsWith('vmess://')) {
    try {
      var jsonStr = atob(uri.substring(8));
      var json = JSON.parse(jsonStr);
      return {
        id: genId(),
        name: json.ps || json.remark || 'VMess',
        type: 'vmess', server: json.add || '', port: parseInt(json.port) || 0,
        password: json.id || '', cipher: json.scy || 'auto',
        tls: json.tls === 'tls' || json.tls === true,
      };
    } catch (e) { return null; }
  }

  // trojan://
  if (uri.startsWith('trojan://')) {
    try {
      var without = uri.substring(9);
      var atIdx2 = without.indexOf('@');
      if (atIdx2 < 0) return null;
      var pwd = without.substring(0, atIdx2);
      var rest2 = without.substring(atIdx2 + 1);
      var hashIdx2 = rest2.indexOf('#');
      var name2 = hashIdx2 >= 0 ? decodeURIComponent(rest2.substring(hashIdx2 + 1)) : 'Trojan';
      var hp = hashIdx2 >= 0 ? rest2.substring(0, hashIdx2) : rest2;
      hp = hp.split('?')[0]; // strip query
      var parts = hp.split(':');
      if (parts.length < 2) return null;
      return { id: genId(), name: name2, type: 'trojan', server: parts[0], port: parseInt(parts[1]) || 443, password: pwd, cipher: '', tls: true };
    } catch (e) { return null; }
  }

  return null;
}

// --- Manual node modal ---
var btnAddNode = document.getElementById('btn-add-node');
if (btnAddNode) {
  btnAddNode.addEventListener('click', function() {
    // Reset all fields
    document.getElementById('node-type').value = 'ss';
    document.getElementById('node-name').value = '';
    document.getElementById('node-server').value = '';
    document.getElementById('node-port').value = '';
    document.getElementById('node-password').value = '';
    document.getElementById('node-cipher').value = 'aes-256-gcm';
    document.getElementById('node-tls').checked = false;
    document.getElementById('node-uri').value = '';
    document.getElementById('modal-node').style.display = 'flex';
  });
}

var btnNodeCancel = document.getElementById('btn-node-cancel');
if (btnNodeCancel) {
  btnNodeCancel.addEventListener('click', function() {
    document.getElementById('modal-node').style.display = 'none';
  });
}

var btnParseUri = document.getElementById('btn-parse-uri');
if (btnParseUri) {
  btnParseUri.addEventListener('click', function() {
    var uri = document.getElementById('node-uri').value.trim();
    if (!uri) return;
    var parsed = parseShareUri(uri);
    if (!parsed) { alert('无法解析该链接，请检查格式是否正确'); return; }
    document.getElementById('node-name').value = parsed.name || '';
    document.getElementById('node-type').value = parsed.type || 'ss';
    document.getElementById('node-server').value = parsed.server || '';
    document.getElementById('node-port').value = parsed.port || '';
    document.getElementById('node-password').value = parsed.password || '';
    document.getElementById('node-cipher').value = parsed.cipher || 'aes-256-gcm';
    document.getElementById('node-tls').checked = !!parsed.tls;
  });
}

var btnNodeSave = document.getElementById('btn-node-save');
if (btnNodeSave) {
  btnNodeSave.addEventListener('click', function() {
    var name = document.getElementById('node-name').value.trim();
    var server = document.getElementById('node-server').value.trim();
    var port = parseInt(document.getElementById('node-port').value) || 0;
    if (!name) { alert('请填写节点名称'); return; }
    if (!server) { alert('请填写服务器地址'); return; }
    if (!port || port < 1 || port > 65535) { alert('请填写有效端口 (1-65535)'); return; }

    manualNodes.push({
      id: genId(),
      name: name,
      type: document.getElementById('node-type').value,
      server: server,
      port: port,
      password: document.getElementById('node-password').value,
      cipher: document.getElementById('node-cipher').value,
      tls: document.getElementById('node-tls').checked,
    });
    saveLocalStorage();
    document.getElementById('modal-node').style.display = 'none';
    // Refresh node list immediately
    loadNodes();
  });
}

// Close modals on overlay click
document.querySelectorAll('.modal-overlay').forEach(function(overlay) {
  overlay.addEventListener('click', function(e) {
    if (e.target === overlay) overlay.style.display = 'none';
  });
});

// --- Rules view ---
async function loadRules() {
  var container = document.getElementById('rules-list');
  try {
    var data = await safeInvoke('get_rules');
    var rules = data.rules || [];
    if (rules.length === 0) {
      container.innerHTML = '<div class="empty-hint">暂无规则数据</div>';
    } else {
      container.innerHTML = rules.map(function(r) {
        return '<div class="rule-row"><span class="rule-type">' + escapeHtml(r.type) + '</span>'
          + escapeHtml(r.payload) + '<span class="rule-arrow">\u2192</span>' + escapeHtml(r.proxy) + '</div>';
      }).join('');
    }
  } catch (e) {
    container.innerHTML = '<div class="empty-hint">无法获取规则: ' + escapeHtml(String(e)) + '</div>';
  }
}

// --- Connections view ---
async function loadConnections() {
  try {
    var data = await safeInvoke('get_connections');
    var conns = data.connections || [];
    document.getElementById('conn-count').textContent = conns.length;
    if (conns.length === 0) {
      document.getElementById('conn-body').innerHTML = '<tr><td colspan="6" style="text-align:center;color:var(--muted)">暂无活动连接</td></tr>';
    } else {
      document.getElementById('conn-body').innerHTML = conns.map(function(c) {
        var dest = c.metadata ? (c.metadata.host || c.metadata.destinationIP || '') : (c.destination || '');
        var net = (c.metadata && c.metadata.network) || c.network || '';
        var rule = (c.rule || '') + ' ' + (c.rulePayload || '');
        var chain = (c.chains || []).join(' \u2192 ');
        return '<tr><td>' + escapeHtml(dest) + '</td><td>' + escapeHtml(net) + '</td><td>'
          + escapeHtml(rule.trim()) + '</td><td>' + escapeHtml(chain) + '</td><td>'
          + formatBytes(c.upload || 0) + '</td><td>' + formatBytes(c.download || 0) + '</td></tr>';
      }).join('');
    }
  } catch (e) {
    document.getElementById('conn-body').innerHTML = '<tr><td colspan="6" style="text-align:center;color:var(--muted)">无法获取: ' + escapeHtml(String(e)) + '</td></tr>';
  }
}

// --- Logs view ---
var logBuffer = [];
var MAX_LOGS = 500;

function loadLogs() { renderLogs(); }

function addLog(level, msg) {
  logBuffer.unshift({ time: new Date().toLocaleTimeString(), level: level, msg: msg });
  if (logBuffer.length > MAX_LOGS) logBuffer.pop();
  var active = document.querySelector('.view.active');
  if (active && active.id === 'view-logs') renderLogs();
}

function renderLogs() {
  var container = document.getElementById('logs-body');
  if (!container) return;
  if (logBuffer.length === 0) {
    container.innerHTML = '<div class="empty-hint">暂无日志</div>';
    return;
  }
  container.innerHTML = logBuffer.map(function(l) {
    var cls = l.level === 'error' ? 'log-error' : (l.level === 'warning' ? 'log-warn' : '');
    return '<div class="log-row ' + cls + '"><span class="log-time">' + escapeHtml(l.time) + '</span>'
      + '<span class="log-level">[' + escapeHtml(l.level) + ']</span> ' + escapeHtml(l.msg) + '</div>';
  }).join('');
}

// --- Traffic ---
async function loadTraffic() {
  try {
    var data = await safeInvoke('get_traffic');
    setApiStatus(true);
    document.getElementById('traffic-up').textContent = formatBytes(data.up || 0) + '/s';
    document.getElementById('traffic-down').textContent = formatBytes(data.down || 0) + '/s';
  } catch (e) {
    setApiStatus(false);
  }
}

// --- Version ---
async function loadVersion() {
  try {
    var data = await safeInvoke('get_version');
    document.getElementById('about-core').textContent = data.version || '-';
  } catch (e) {}
}

// --- Configs / current mode ---
async function loadConfigs() {
  try {
    var data = await safeInvoke('get_configs');
    var mode = (data.mode || 'rule').toLowerCase();
    document.querySelectorAll('.mode-tab').forEach(function(t) {
      t.classList.toggle('active', t.dataset.mode === mode);
    });
    var sysProxy = await safeInvoke('get_system_proxy');
    document.getElementById('sys-proxy-toggle').checked = !!sysProxy;
    document.getElementById('allow-lan-toggle').checked = !!data['allow-lan'];
  } catch (e) {}
}

// --- API config handlers ---
var saveApiBtn = document.getElementById('save-api-btn');
if (saveApiBtn) {
  saveApiBtn.addEventListener('click', async function() {
    var baseUrl = document.getElementById('api-url-input').value.trim();
    var secret = document.getElementById('api-secret-input').value;
    try {
      await safeInvoke('set_api_config', { base_url: baseUrl, secret: secret });
      // Save to localStorage too
      if (baseUrl) localStorage.setItem('redog_api_url', baseUrl);
      localStorage.setItem('redog_api_secret', secret);
      // Test connection
      var data = await safeInvoke('get_version');
      setApiStatus(true);
      var detail = document.getElementById('api-status-detail');
      if (detail) detail.textContent = '已连接 - ' + (data.version || 'OK');
      // Refresh proxies
      loadProxies();
    } catch (e) {
      setApiStatus(false);
      var detail = document.getElementById('api-status-detail');
      if (detail) detail.textContent = '连接失败: ' + e;
    }
  });
}

async function loadApiConfig() {
  try {
    var cfg = await safeInvoke('get_api_config');
    var urlInput = document.getElementById('api-url-input');
    if (urlInput && cfg.base_url) urlInput.value = cfg.base_url;
    var secretInput = document.getElementById('api-secret-input');
    if (secretInput && cfg.has_secret) secretInput.placeholder = '已设置 (自动检测)';
  } catch (e) {}
}

// --- Settings handlers ---
document.getElementById('sys-proxy-toggle').addEventListener('change', async function(e) {
  try {
    await safeInvoke('set_system_proxy', { enable: e.target.checked });
  } catch (err) {
    alert('设置系统代理失败: ' + err);
    e.target.checked = !e.target.checked;
  }
});
document.getElementById('allow-lan-toggle').addEventListener('change', async function(e) {
  try { await safeInvoke('patch_configs', { body: { 'allow-lan': e.target.checked } }); }
  catch (err) { console.warn(err); }
});
document.getElementById('copy-cmd-btn').addEventListener('click', async function() {
  try {
    var cmd = await safeInvoke('copy_terminal_command');
    alert('已复制到剪贴板:\n' + cmd);
  } catch (e) { alert('失败: ' + e); }
});
document.getElementById('reload-btn').addEventListener('click', async function() {
  try {
    await safeInvoke('patch_configs', { body: { path: '' } });
    alert('已重载');
  } catch (e) { alert('失败: ' + e); }
});

// --- Helpers ---
function escapeHtml(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, function(c) {
    return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
  });
}
function formatBytes(n) {
  if (n < 1024) return n + ' B';
  if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
  if (n < 1073741824) return (n / 1048576).toFixed(1) + ' MB';
  return (n / 1073741824).toFixed(2) + ' GB';
}
function genId() {
  return Date.now().toString(36) + Math.random().toString(36).substring(2, 8);
}
function truncateUrl(url) {
  return url.length > 60 ? url.substring(0, 57) + '...' : url;
}

function refreshView(view) {
  switch (view) {
    case 'proxies': loadProxies(); break;
    case 'nodes': loadNodes(); break;
    case 'rules': loadRules(); break;
    case 'connections': loadConnections(); break;
    case 'logs': loadLogs(); break;
    case 'settings': loadConfigs(); break;
    case 'about': loadVersion(); break;
  }
}

// --- Polling ---
setInterval(loadTraffic, 1000);
setInterval(function() {
  var active = document.querySelector('.view.active');
  if (!active) return;
  if (active.id === 'view-connections') loadConnections();
  else if (active.id === 'view-proxies') loadProxies();
}, 2000);

// --- Initial load ---
loadApiConfig();
loadConfigs();
loadProxies();
loadTraffic();
loadVersion();
