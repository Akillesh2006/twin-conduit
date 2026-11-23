@echo off
title OVERDRIVE - 2-Player Monster Defense
cd /d "%~dp0"
mode con: cols=100 lines=32
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0run.ps1"
if %errorlevel% neq 0 (
    node tools\play-cli.mjs
)

