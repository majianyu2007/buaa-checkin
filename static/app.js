// ── State ─────────────────────────────────────────────────────────────────────

const API = '';
let token = localStorage.getItem('token');
let currentUser = JSON.parse(localStorage.getItem('currentUser') || 'null');
let activeTab = 'schedules';

// ── Bootstrap ────────────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  if (token && currentUser) {
    showDashboard();
  } else {
    showLogin();
  }
});

// ── API helpers ──────────────────────────────────────────────────────────────

async function api(method, path, body) {
  const opts = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (token) opts.headers['Authorization'] = `Bearer ${token}`;
  if (body) opts.body = JSON.stringify(body);

  const res = await fetch(`${API}${path}`, opts);
  const data = await res.json();
  if (!res.ok) throw new Error(data.message || '请求失败');
  return data;
}

// ── Toast ────────────────────────────────────────────────────────────────────

function toast(message, type = 'success') {
  let container = document.getElementById('toast-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'toast-container';
    container.className = 'toast-container';
    document.body.appendChild(container);
  }
  const el = document.createElement('div');
  el.className = `toast toast-${type}`;
  el.textContent = message;
  container.appendChild(el);
  setTimeout(() => el.remove(), 3500);
}

// ── Views ────────────────────────────────────────────────────────────────────

function showLogin() {
  const app = document.getElementById('app');
  app.innerHTML = `
    <div class="header">
      <h1>BUAA 智慧教室</h1>
      <p>自动签到管理系统</p>
    </div>
    <div class="login-container">
      <div class="card">
        <div class="card-title"><span class="icon">🔐</span> 登录</div>
        <form id="login-form">
          <div class="form-group">
            <label for="student-id">学号</label>
            <input type="text" id="student-id" placeholder="请输入您的学号" autocomplete="off" required>
          </div>
          <button type="submit" class="btn btn-primary btn-block" id="login-btn">
            登录
          </button>
        </form>
      </div>
    </div>
  `;
  document.getElementById('login-form').addEventListener('submit', handleLogin);
}

async function handleLogin(e) {
  e.preventDefault();
  const btn = document.getElementById('login-btn');
  const studentId = document.getElementById('student-id').value.trim();
  if (!studentId) return;

  btn.disabled = true;
  btn.innerHTML = '<span class="spinner"></span> 登录中...';

  try {
    const data = await api('POST', '/api/login', { student_id: studentId });
    token = data.token;
    currentUser = { student_id: data.student_id, name: data.name };
    localStorage.setItem('token', token);
    localStorage.setItem('currentUser', JSON.stringify(currentUser));
    toast(`欢迎, ${data.name}`);
    showDashboard();
  } catch (err) {
    toast(err.message, 'error');
    btn.disabled = false;
    btn.textContent = '登录';
  }
}

async function showDashboard() {
  const app = document.getElementById('app');
  app.innerHTML = '<div class="empty"><span class="spinner"></span> 载入系统信息...</div>';

  let isAdmin = false;
  let version = "0.0.0";
  try {
    const sys = await api('GET', '/api/system/info');
    isAdmin = sys.is_admin;
    version = sys.version;

    // Async update check
    fetch('https://api.github.com/repos/majianyu2007/buaa-checkin/releases/latest')
      .then(res => res.json())
      .then(rel => {
        if (rel.tag_name && rel.tag_name > 'v' + version) {
          const badge = document.getElementById('update-badge');
          if (badge) {
            badge.innerHTML = `<a href="${rel.html_url}" target="_blank" style="color:#e74c3c;font-size:0.8rem;text-decoration:none;background:#fdebd0;padding:2px 6px;border-radius:4px;margin-left:8px;">✨ 有新版本 ${rel.tag_name}</a>`;
          }
        }
      }).catch(() => {});
  } catch (err) {
    if (err.message.includes("过期") || err.message.includes("Token")) {
      return logout();
    }
    toast("无法获取系统信息: " + err.message, "error");
  }

  const initial = currentUser.name ? currentUser.name[0] : '?';
  app.innerHTML = `
    <div class="header" style="position:relative">
      <h1>BUAA 智慧教室</h1>
      <p>自动签到管理系统 <span id="update-badge" style="display:inline-block">v${version}</span></p>
    </div>
    <div class="navbar">
      <div class="navbar-user">
        <div class="avatar">${initial}</div>
        <span>${currentUser.name} (${currentUser.student_id}) ${isAdmin ? '<small style="color:#f39c12">[管理员]</small>' : ''}</span>
      </div>
      <button class="btn btn-secondary btn-sm" onclick="logout()">退出登录</button>
    </div>
    <div class="tabs">
      <button class="tab active" data-tab="schedules" onclick="switchTab('schedules')">📋 今日课表</button>
      <button class="tab" data-tab="all-courses" onclick="switchTab('all-courses')">📚 我的课程</button>
      ${isAdmin ? `
      <button class="tab" data-tab="users" onclick="switchTab('users')">👥 用户管理</button>
      <button class="tab" data-tab="tasks" onclick="switchTab('tasks')">⏱ 自动任务</button>
      <button class="tab" data-tab="webhook" onclick="switchTab('webhook')">🔔 通知设置</button>
      <button class="tab" data-tab="settings" onclick="switchTab('settings')">⚙️ 系统设置</button>
      ` : ''}
    </div>
    <div id="tab-content"></div>
  `;
  switchTab('schedules');
}

function logout() {
  token = null;
  currentUser = null;
  localStorage.removeItem('token');
  localStorage.removeItem('currentUser');
  showLogin();
}

// ── Tabs ─────────────────────────────────────────────────────────────────────

function switchTab(tab) {
  activeTab = tab;
  document.querySelectorAll('.tab').forEach(t => {
    t.classList.toggle('active', t.dataset.tab === tab);
  });
  const content = document.getElementById('tab-content');
  content.innerHTML = '<div class="empty"><span class="spinner"></span></div>';

  switch (tab) {
    case 'schedules': loadSchedules(); break;
    case 'all-courses': loadAllCourses(); break;
    case 'users': loadUsers(); break;
    case 'tasks': loadTasks(); break;
    case 'webhook': loadWebhook(); break;
    case 'settings': loadSettings(); break;
  }
}

// ── Schedules tab ────────────────────────────────────────────────────────────

async function loadSchedules(dateStr) {
  const content = document.getElementById('tab-content');
  const date = dateStr || new Date().toISOString().slice(0, 10).replace(/-/g, '');
  const displayDate = date.slice(0, 4) + '-' + date.slice(4, 6) + '-' + date.slice(6, 8);

  try {
    const [schedules, enabledCourses] = await Promise.all([
      api('GET', `/api/schedules?date=${date}`),
      api('GET', '/api/me/courses').catch(() => [])
    ]);

    let html = `
      <div class="card">
        <div class="card-title" style="justify-content:space-between">
          <span><span class="icon">📋</span> 课表查询</span>
          <input type="date" value="${displayDate}" onchange="loadSchedules(this.value.replace(/-/g, ''))" 
            style="padding:4px 8px; border:1px solid #ddd; border-radius:4px; font-size:14px; background:var(--bg-secondary); color:var(--text-primary)">
        </div>
    `;

    if (schedules.length === 0) {
      html += `
        <div class="empty">
          <div class="icon">📭</div>
          <p>${displayDate} 没有课程安排</p>
        </div>
      </div>`;
      content.innerHTML = html;
      return;
    }

    html += `
        <table class="schedule-table">
          <thead>
            <tr>
              <th>课程</th>
              <th>教师</th>
              <th>时间</th>
              <th>状态</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            ${schedules.map(s => {
              const isAuto = enabledCourses.includes(s.course_id);
              return `
              <tr>
                <td>${escHtml(s.courseName || s.name)}</td>
                <td>${escHtml(s.teacherName || s.teacher)}</td>
                <td>${formatTime(s.classBeginTime || s.time)}</td>
                <td>${s.signStatus === '1' || s.status_raw === '1'
                    ? '<span class="badge badge-signed">✓ 已签到</span>'
                    : '<span class="badge badge-unsigned">○ 未签到</span>'}</td>
                <td>
                  ${s.signStatus !== '1' && s.status_raw !== '1'
                    ? `<button class="btn btn-primary btn-sm" onclick="doCheckin('${escAttr(s.id)}', '${date}')">签到</button>`
                    : ''}
                  <button class="btn btn-sm ${isAuto ? 'btn-toggle-active' : 'btn-toggle'}" 
                    onclick="toggleAutoCheckin('${escAttr(s.course_id)}', ${isAuto}, 'schedules', '${date}')"
                    style="margin-left:4px">
                    ${isAuto ? '✅ 已开启' : '⭕️ 开启自动'}
                  </button>
                </td>
              </tr>
              `;
            }).join('')}
          </tbody>
        </table>
      </div>
    `;
    content.innerHTML = html;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function loadAllCourses() {
  const content = document.getElementById('tab-content');
  try {
    const [courses, enabledCourses] = await Promise.all([
      api('GET', '/api/me/courses/all'),
      api('GET', '/api/me/courses').catch(() => [])
    ]);

    content.innerHTML = `
      <div class="card">
        <div class="card-title" style="justify-content:space-between">
          <span><span class="icon">📚</span> 全量课程列表 (本学期)</span>
          <button class="btn btn-primary btn-sm" onclick="enableAllCourses()">🚀 一键开启全部代签</button>
        </div>
        <p style="color:var(--text-secondary);font-size:0.88rem;margin-bottom:1rem">在此处开启自动签到后，系统将自动为该课程的所有未来小节打卡。</p>
        ${courses.length === 0 ? '<div class="empty"><div class="icon">📚</div><p>未找到选课记录</p></div>' : `
        <table class="schedule-table">
          <thead>
            <tr>
              <th>课程名称</th>
              <th>授课教师</th>
              <th>自动签到</th>
            </tr>
          </thead>
          <tbody>
            ${courses.map(c => {
              const isAuto = enabledCourses.includes(c.id);
              const name = c.course_name || c.name;
              const teacher = c.teacher_name || c.teacher;
              return `
              <tr>
                <td>${escHtml(name)}</td>
                <td>${escHtml(teacher)}</td>
                <td>
                  <button class="btn btn-sm ${isAuto ? 'btn-toggle-active' : 'btn-toggle'}" 
                    onclick="toggleAutoCheckin('${escAttr(c.id)}', ${isAuto}, 'all-courses')">
                    ${isAuto ? '✅ 自动签到 (已开启)' : '⭕️ 开启自动代签'}
                  </button>
                </td>
              </tr>
              `;
            }).join('')}
          </tbody>
        </table>
        `}
      </div>
    `;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function doCheckin(scheduleId, date) {
  try {
    const data = await api('POST', '/api/checkin', { schedule_id: scheduleId });
    toast(data.message);
    setTimeout(() => loadSchedules(date), 1500);
  } catch (err) {
    toast(err.message, 'error');
  }
}

async function toggleAutoCheckin(courseId, isCurrentlyEnabled, source, date) {
  try {
    const method = isCurrentlyEnabled ? 'DELETE' : 'POST';
    const data = await api(method, `/api/me/courses/${courseId}`);
    toast(data.message);
    // Reload the originating view
    setTimeout(() => {
      if (source === 'all-courses') loadAllCourses();
      else loadSchedules(date);
    }, 300);
  } catch (err) {
    toast(err.message, 'error');
  }
}

async function enableAllCourses() {
  if (!confirm('确定要开启本学期所有课程的自动签到吗？')) return;
  try {
    const data = await api('POST', '/api/me/courses/all');
    toast(data.message);
    setTimeout(() => loadAllCourses(), 300);
  } catch (err) {
    toast(err.message, 'error');
  }
}

// ── Settings tab ─────────────────────────────────────────────────────────────

async function loadSettings() {
  const content = document.getElementById('tab-content');
  content.innerHTML = '<div class="card"><div class="empty"><span class="spinner"></span> 正在获取配置...</div></div>';
  
  try {
    const config = await api('GET', '/api/system/settings');
    content.innerHTML = `
      <div class="card">
        <div class="card-title"><span class="icon">⚙️</span> 系统配置</div>
        <div class="form-group">
          <label>服务端口 (Port)</label>
          <input type="number" id="setting-port" value="${config.port || ''}" placeholder="例如 3000">
          <p class="help-text">修改端口后需要重启程序才能生效。</p>
        </div>
        <div class="form-group">
          <label>管理员学号 (Admin ID)</label>
          <input type="text" id="setting-admin" value="${escAttr(config.admin_id || '')}" placeholder="管理员学号">
          <p class="help-text">修改此项将变更系统唯一的管理权限拥有者。</p>
        </div>
        <div style="margin-top:1.5rem">
          <button class="btn btn-primary" onclick="updateSettings()">保存配置</button>
        </div>
      </div>

      <div class="card" style="border-color:rgba(231, 76, 60, 0.3)">
        <div class="card-title" style="color:var(--accent-red)"><span class="icon">⚠️</span> 电源管理</div>
        <p style="color:var(--text-secondary);font-size:0.88rem;margin-bottom:1rem">
          如果是使用 Docker 或 Systemd 管理的服务，关闭系统后通常会自动重启。
        </p>
        <div style="display:flex;gap:10px">
          <button class="btn btn-danger" onclick="shutdownSystem()">关闭系统</button>
        </div>
      </div>
    `;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">无法获取配置: ${escHtml(err.message)}</div></div>`;
  }
}

async function updateSettings() {
  const port = parseInt(document.getElementById('setting-port').value);
  const admin_id = document.getElementById('setting-admin').value;
  try {
    const data = await api('POST', '/api/system/settings', { port, admin_id });
    toast(data.message);
  } catch (err) {
    toast(err.message, 'error');
  }
}

async function shutdownSystem() {
  if (!confirm('确定要关闭系统吗？\n警告：关闭后您需要手动在服务器上重新启动，除非配置了自动重启。')) return;
  try {
    const data = await api('POST', '/api/system/shutdown');
    toast(data.message);
    setTimeout(() => {
      window.location.reload();
    }, 2000);
  } catch (err) {
    toast(err.message, 'error');
  }
}

// ── Users tab ────────────────────────────────────────────────────────────────

async function loadUsers() {
  const content = document.getElementById('tab-content');
  try {
    const users = await api('GET', '/api/users');
    content.innerHTML = `
      <div class="card">
        <div class="card-title" style="margin-bottom:1rem">
          <span class="icon">👥</span> 已注册系统用户 (${users.length})
        </div>
        <p style="color:var(--text-secondary);font-size:0.88rem;margin-bottom:1rem">
          用户可通过“今日课表”自行开启自动签到功能，开启后将显示在此列表中。你可以移除不需要自动代签的用户。
        </p>
        ${users.length === 0 ? '<div class="empty"><div class="icon">👤</div><p>暂无注册用户</p></div>' :
          users.map(u => `
            <div class="user-item">
              <div class="user-info">
                <span class="user-name">${escHtml(u.name)}</span>
                <span class="user-id">${escHtml(u.student_id)}</span>
                <span class="user-courses">正在自动打卡课程数: ${u.course_ids.length}</span>
              </div>
              <button class="btn btn-danger btn-sm" onclick="removeUser('${escAttr(u.student_id)}')">移除</button>
            </div>
          `).join('')
        }
      </div>
    `;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function removeUser(studentId) {
  if (!confirm(`确定要删除用户 ${studentId} 吗？`)) return;
  try {
    const data = await api('DELETE', `/api/users/${studentId}`);
    toast(data.message);
    loadUsers();
  } catch (err) {
    toast(err.message, 'error');
  }
}

// ── Tasks tab ────────────────────────────────────────────────────────────────

async function loadTasks() {
  const content = document.getElementById('tab-content');
  try {
    const tasks = await api('GET', '/api/tasks');
    content.innerHTML = `
      <div class="card">
        <div class="card-title" style="justify-content:space-between">
          <span><span class="icon">⏱</span> 待执行任务 (${tasks.length})</span>
          <button class="btn btn-secondary btn-sm" onclick="triggerPoll()">🔄 手动轮询</button>
        </div>
        ${tasks.length === 0 ? '<div class="empty"><div class="icon">✨</div><p>当前没有待执行的签到任务</p></div>' :
          tasks.map(t => `
            <div class="task-item">
              <span class="task-time">${formatUnixTime(t.run_at)}</span>
              <span class="task-detail">
                <span class="badge badge-pending">学号 ${escHtml(t.student_id)}</span>
                课程 ${escHtml(t.course_id)}
              </span>
            </div>
          `).join('')
        }
      </div>
    `;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function triggerPoll() {
  try {
    const data = await api('POST', '/api/poll');
    toast(data.message);
    setTimeout(() => loadTasks(), 1000);
  } catch (err) {
    toast(err.message, 'error');
  }
}

// ── Webhook tab ──────────────────────────────────────────────────────────────

async function loadWebhook() {
  const content = document.getElementById('tab-content');
  try {
    const config = await api('GET', '/api/webhook');
    content.innerHTML = `
      <div class="card">
        <div class="card-title"><span class="icon">🔔</span> Webhook 通知设置</div>
        <p style="color:var(--text-secondary);font-size:0.88rem;margin-bottom:1rem">签到成功时自动推送通知到您的手机。</p>
        <div class="form-group">
          <label>启用通知</label>
          <select id="wh-enabled">
            <option value="true" ${config.enabled ? 'selected' : ''}>已启用</option>
            <option value="false" ${!config.enabled ? 'selected' : ''}>已禁用</option>
          </select>
        </div>
        <div class="form-group">
          <label>通知渠道</label>
          <select id="wh-provider">
            <option value="serverchan" ${config.provider === 'serverchan' ? 'selected' : ''}>Server酱</option>
            <option value="custom" ${config.provider === 'custom' ? 'selected' : ''}>自定义 Webhook</option>
          </select>
        </div>
        <div class="form-group">
          <label id="wh-key-label">${config.provider === 'serverchan' ? 'Server酱 SendKey' : 'Webhook URL'}</label>
          <input type="text" id="wh-key" value="${escAttr(config.key)}" placeholder="${config.provider === 'serverchan' ? 'SCT...' : 'https://...'}">
        </div>
        <div class="form-group" id="wh-url-group" style="${config.provider === 'custom' ? '' : 'display:none'}">
          <label>自定义 URL（可选，覆盖 Key）</label>
          <input type="text" id="wh-url" value="${escAttr(config.url || '')}" placeholder="https://your-webhook-endpoint.com">
        </div>
        <div class="form-group" style="display:flex;gap:10px">
          <button class="btn btn-primary" onclick="saveWebhook()" style="flex:1">保存设置</button>
          <button class="btn btn-secondary" onclick="testWebhook()" style="flex:1">测试通知</button>
        </div>
      </div>
    `;
    document.getElementById('wh-provider').addEventListener('change', (e) => {
      const isSC = e.target.value === 'serverchan';
      document.getElementById('wh-key-label').textContent = isSC ? 'Server酱 SendKey' : 'Webhook URL';
      document.getElementById('wh-key').placeholder = isSC ? 'SCT...' : 'https://...';
      document.getElementById('wh-url-group').style.display = isSC ? 'none' : '';
    });
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function saveWebhook() {
  const config = {
    enabled: document.getElementById('wh-enabled').value === 'true',
    provider: document.getElementById('wh-provider').value,
    key: document.getElementById('wh-key').value.trim(),
    url: document.getElementById('wh-url') ? document.getElementById('wh-url').value.trim() || null : null,
  };
  try {
    const data = await api('POST', '/api/webhook', config);
    toast(data.message);
  } catch (err) {
    toast(err.message, 'error');
  }
}

async function testWebhook() {
  try {
    const data = await api('POST', '/api/webhook/test');
    toast(data.message);
  } catch (err) {
    toast(err.message, 'error');
  }
}

// ── Utilities ────────────────────────────────────────────────────────────────

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s || '';
  return d.innerHTML;
}

function escAttr(s) {
  return (s || '').replace(/'/g, "\\'").replace(/"/g, '&quot;');
}

function formatTime(s) {
  if (!s) return '-';
  // "2024-03-01 08:00:00" → "08:00"
  const match = s.match(/(\d{2}:\d{2})/);
  return match ? match[1] : s;
}

function formatUnixTime(ts) {
  const d = new Date(ts * 1000);
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  const ss = String(d.getSeconds()).padStart(2, '0');
  return `${hh}:${mm}:${ss}`;
}
