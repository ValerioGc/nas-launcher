import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';

const port = Number(process.argv[2] ?? 3003);
const ACCOUNT = { username: 'admin', password: 'long enough' };
const COOKIE = 'nas_session';

const sessions = new Set();
let policy = { enabled: true, attempts: 5, lockMinutes: 1 };

function send(answer, status, body, headers = {}) {
  answer.writeHead(status, {
    'cache-control': 'no-store',
    ...(body === undefined ? {} : { 'content-type': 'application/json' }),
    ...headers,
  });
  answer.end(body === undefined ? undefined : JSON.stringify(body));
}

function session(request) {
  const token = (request.headers.cookie ?? '')
    .split(';')
    .map((part) => part.trim().split('='))
    .find(([name]) => name === COOKIE)?.[1];
  return token && sessions.has(token) ? token : null;
}

async function json(request) {
  let raw = '';
  for await (const chunk of request) {
    raw += chunk;
  }
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function validPolicy(value) {
  return (
    typeof value?.enabled === 'boolean' &&
    Number.isInteger(value.attempts) &&
    value.attempts >= 1 &&
    value.attempts <= 20 &&
    Number.isInteger(value.lockMinutes) &&
    value.lockMinutes >= 1 &&
    value.lockMinutes <= 1440
  );
}

createServer(async (request, answer) => {
  const { method, url } = request;

  if (url === '/health') {
    send(answer, 200, { status: 'ok' });
    return;
  }

  if (method === 'POST' && url === '/auth/login') {
    const input = await json(request);
    if (input?.username?.trim() !== ACCOUNT.username || input?.password !== ACCOUNT.password) {
      send(answer, 401);
      return;
    }
    const token = randomBytes(32).toString('hex');
    sessions.add(token);
    send(
      answer,
      200,
      {
        username: ACCOUNT.username,

        device: null,
      },
      { 'set-cookie': `${COOKIE}=${token}; HttpOnly; SameSite=Strict; Path=/` },
    );
    return;
  }

  if (method === 'GET' && url === '/auth/session') {
    if (session(request)) {
      send(answer, 200, { authenticated: true, username: ACCOUNT.username });
    } else {
      send(answer, 401);
    }
    return;
  }

  if (method === 'POST' && url === '/auth/logout') {
    sessions.delete(session(request));
    send(answer, 204, undefined, {
      'set-cookie': `${COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0`,
    });
    return;
  }

  if (url === '/auth/policy' && (method === 'GET' || method === 'PUT')) {
    if (!session(request)) {
      send(answer, 401);
      return;
    }
    if (method === 'PUT') {
      const input = await json(request);
      if (!validPolicy(input)) {
        send(answer, 400);
        return;
      }
      policy = {
        enabled: input.enabled,
        attempts: input.attempts,
        lockMinutes: input.lockMinutes,
      };
    }
    send(answer, 200, policy);
    return;
  }

  send(answer, 404, { error: 'not found' });
}).listen(port, '127.0.0.1');
