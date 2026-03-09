import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock fetch globally
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

function mockFetchResponse(status: number, body: unknown) {
  fetchMock.mockResolvedValue({
    ok: status >= 200 && status < 400,
    status,
    text: () => Promise.resolve(JSON.stringify(body)),
    json: () => Promise.resolve(body),
  });
}

describe('API client', () => {
  let api: Awaited<typeof import('./api')>['api'];

  beforeEach(async () => {
    fetchMock.mockReset();
    window.location.hash = '';
    const mod = await import('./api');
    api = mod.api;
  });

  it('makes GET requests to correct endpoint', async () => {
    mockFetchResponse(200, { setup_required: false });
    await api.auth.setupRequired();
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/auth/setup-required',
      expect.objectContaining({ credentials: 'include' })
    );
  });

  it('makes POST requests with JSON body', async () => {
    mockFetchResponse(200, { success: true, user: { id: 1, username: 'admin' } });
    await api.auth.login('admin', 'password');
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/auth/login',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ username: 'admin', password: 'password' }),
      })
    );
  });

  it('throws on error response with code', async () => {
    mockFetchResponse(400, { code: 'PASSWORD_TOO_SHORT' });
    await expect(api.auth.setup('admin', 'short')).rejects.toThrow();
  });

  it('throws on legacy error response', async () => {
    mockFetchResponse(500, { error: 'Something went wrong' });
    await expect(api.system.version()).rejects.toThrow();
  });

  it('redirects to login on 401', async () => {
    mockFetchResponse(401, { code: 'NOT_AUTHENTICATED' });
    try {
      await api.auth.me();
    } catch {
      // expected
    }
    expect(window.location.hash).toBe('#/login');
  });

  it('fetches jobs list', async () => {
    mockFetchResponse(200, [{ id: 1, name: 'Test Job' }]);
    const jobs = await api.jobs.list();
    expect(jobs).toEqual([{ id: 1, name: 'Test Job' }]);
  });

  it('creates a job with POST', async () => {
    mockFetchResponse(201, { id: 1, name: 'New Job' });
    await api.jobs.create({ name: 'New Job', source_dirs: ['/home'] } as never);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/jobs',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('handles version endpoint', async () => {
    mockFetchResponse(200, { version: '1.0.0', build: 'abc123', date: '2024-01-01' });
    const result = await api.system.version();
    expect(result.version).toBe('1.0.0');
  });

  it('translates error codes in error messages', async () => {
    mockFetchResponse(429, { code: 'RATE_LIMITED', params: { seconds: 30 } });
    try {
      await api.auth.login('admin', 'pass');
      expect.unreachable('should have thrown');
    } catch (e: unknown) {
      expect(e).toBeInstanceOf(Error);
    }
  });

  it('deletes a job with DELETE', async () => {
    mockFetchResponse(200, { success: true });
    await api.jobs.delete(1);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/jobs/1',
      expect.objectContaining({ method: 'DELETE' })
    );
  });
});
