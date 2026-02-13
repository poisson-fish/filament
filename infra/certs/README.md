To enable SSL with Cloudflare Full (Strict) mode:

1. Go to Cloudflare Dashboard -> SSL/TLS -> Origin Server.
2. Click "Create Certificate".
3. Keep default settings (RSA, 15 years, etc).
4. Copy the "Origin Certificate" content into a file named `cert.pem` in this directory.
5. Copy the "Private Key" content into a file named `key.pem` in this directory.

After adding these files, restart the Caddy container:
docker compose -f ../infra/docker-compose.yml restart reverse-proxy
