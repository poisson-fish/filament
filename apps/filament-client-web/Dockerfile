FROM node:22-alpine AS build
WORKDIR /app

ARG VITE_FILAMENT_HCAPTCHA_SITE_KEY
ENV VITE_FILAMENT_HCAPTCHA_SITE_KEY=${VITE_FILAMENT_HCAPTCHA_SITE_KEY}

COPY apps/filament-client-web/package.json /app/package.json
COPY apps/filament-client-web/package-lock.json /app/package-lock.json
RUN npm ci

COPY apps/filament-client-web /app
RUN npm run build

FROM nginxinc/nginx-unprivileged:1.27-alpine
COPY apps/filament-client-web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html

EXPOSE 4173
