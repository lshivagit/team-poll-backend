# Team Poll - Real-Time Decision App

A real-time team polling application built with Rust (Axum) and Next.js (App Router). Features live result streaming, authentication, and optimized performance.

## Setup

Backend (Rust / Axum)
Path: team-poll-backend-main/

Requirements:
- Rust
- MySQL

Steps:
1. Create database:
   CREATE DATABASE team_poll;

3. Create .env file:
   DATABASE_URL=mysql://username:password@localhost/team_poll
   PORT=3000

4. Run migrations (if using SQLx):
   sqlx migrate run

5. Start server:
   cargo run

Backend runs on http://localhost:3000

Frontend (Next.js)
Path: team-poll-frontend-main/

Steps:
1. Install:
   npm install

2. Create .env.local:
   NEXT_PUBLIC_API_URL=http://localhost:3000

3. Start:
   npm run dev

Frontend runs on http://localhost:3001

## Architecture

Real-time updates use Server-Sent Events (SSE). It is HTTP-based, lightweight, and supports auto-reconnect. Suitable for one-way updates like live vote counts.

Performance optimizations:
- Lazy loading SSE using Intersection Observer so connections open only when polls are visible
- Fast-fetch plus stream approach: initial GET request loads data instantly, SSE handles updates

## Decisions

- Security: validate team membership on every SSE connection
- Database: MySQL for reliability
- UI: single-column layout for better readability

## Work Summary

- Fixed Next.js build issue using Suspense
- Implemented lazy-loaded SSE streams
- Added fast-fetch to remove initial lag
- Improved UI layout
- Added documentation

## Status

Complete and production-ready
