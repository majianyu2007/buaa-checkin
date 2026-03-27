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

function showDashboard() {
  const app = document.getElementById('app');
  const initial = currentUser.name ? currentUser.name[0] : '?';
  app.innerHTML = `
    <div class="header">
      <h1>BUAA 智慧教室</h1>
      <p>自动签到管理系统</p>
    </div>
    <div class="navbar">
      <div class="navbar-user">
        <div class="avatar">${initial}</div>
        <span>${currentUser.name} (${currentUser.student_id})</span>
      </div>
      <button class="btn btn-secondary btn-sm" onclick="logout()">退出登录</button>
    </div>
    <div class="tabs">
      <button class="tab active" data-tab="schedules" onclick="switchTab('schedules')">📋 今日课表</button>
      <button class="tab" data-tab="users" onclick="switchTab('users')">👥 用户管理</button>
      <button class="tab" data-tab="tasks" onclick="switchTab('tasks')">⏱ 自动任务</button>
      <button class="tab" data-tab="webhook" onclick="switchTab('webhook')">🔔 通知设置</button>
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
    case 'users': loadUsers(); break;
    case 'tasks': loadTasks(); break;
    case 'webhook': loadWebhook(); break;
  }
}

// ── Schedules tab ────────────────────────────────────────────────────────────

async function loadSchedules() {
  const content = document.getElementById('tab-content');
  try {
    const schedules = await api('GET', '/api/schedules');
    if (schedules.length === 0) {
      content.innerHTML = `
        <div class="card">
          <div class="card-title"><span class="icon">📋</span> 今日课表</div>
          <div class="empty">
            <div class="icon">📭</div>
            <p>今天没有课程安排</p>
          </div>
        </div>
      `;
      return;
    }
    content.innerHTML = `
      <div class="card">
        <div class="card-title"><span class="icon">📋</span> 今日课表</div>
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
            ${schedules.map(s => `
              <tr>
                <td>${escHtml(s.courseName || s.name)}</td>
                <td>${escHtml(s.teacherName || s.teacher)}</td>
                <td>${formatTime(s.classBeginTime || s.time)}</td>
                <td>${s.signStatus === '1' || s.status_raw === '1'
                    ? '<span class="badge badge-signed">✓ 已签到</span>'
                    : '<span class="badge badge-unsigned">○ 未签到</span>'}</td>
                <td>
                  ${s.signStatus !== '1' && s.status_raw !== '1'
                    ? `<button class="btn btn-primary btn-sm" onclick="doCheckin('${escAttr(s.id)}')">签到</button>`
                    : ''}
                </td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    `;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function doCheckin(scheduleId) {
  try {
    const data = await api('POST', '/api/checkin', { schedule_id: scheduleId });
    toast(data.message);
    loadSchedules();
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
        <div class="card-title"><span class="icon">➕</span> 添加用户到自动签到</div>
        <div class="inline-form" id="add-user-form">
          <div class="form-group">
            <label>学号</label>
            <input type="text" id="new-student-id" placeholder="学号">
          </div>
          <div class="form-group">
            <label>课程 ID（逗号分隔）</label>
            <input type="text" id="new-course-ids" placeholder="course1,course2">
          </div>
          <button class="btn btn-primary btn-sm" onclick="addUser()" style="margin-bottom:0">添加</button>
        </div>
      </div>
      <div class="card">
        <div class="card-title"><span class="icon">👥</span> 已注册用户 (${users.length})</div>
        ${users.length === 0 ? '<div class="empty"><div class="icon">👤</div><p>暂无注册用户</p></div>' :
          users.map(u => `
            <div class="user-item">
              <div class="user-info">
                <span class="user-name">${escHtml(u.name)}</span>
                <span class="user-id">${escHtml(u.student_id)}</span>
                <span class="user-courses">课程: ${u.course_ids.length ? u.course_ids.map(escHtml).join(', ') : '无'}</span>
              </div>
              <button class="btn btn-danger btn-sm" onclick="removeUser('${escAttr(u.student_id)}')">删除</button>
            </div>
          `).join('')
        }
      </div>
    `;
  } catch (err) {
    content.innerHTML = `<div class="card"><div class="empty">加载失败: ${escHtml(err.message)}</div></div>`;
  }
}

async function addUser() {
  const studentId = document.getElementById('new-student-id').value.trim();
  const courseIdsRaw = document.getElementById('new-course-ids').value.trim();
  if (!studentId) { toast('请输入学号', 'error'); return; }
  const courseIds = courseIdsRaw ? courseIdsRaw.split(',').map(s => s.trim()).filter(Boolean) : [];
  try {
    const data = await api('POST', '/api/users', { student_id: studentId, course_ids: courseIds });
    toast(data.message);
    loadUsers();
  } catch (err) {
    toast(err.message, 'error');
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
        <button class="btn btn-primary" onclick="saveWebhook()">保存设置</button>
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
