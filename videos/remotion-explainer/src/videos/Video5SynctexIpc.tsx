import React from 'react';
import { useCurrentFrame, useVideoConfig } from 'remotion';
import timingsJson from '../generated/timings.json';
import type { TimingsManifest } from '../types';
import { FONT_MONO, FONT_SANS } from '../components/StageFrame';
import { PinleafWindow } from '../components/PinleafWindow';
import { VideoSequenceShell } from '../components/VideoSequenceShell';

const manifest = timingsJson as TimingsManifest;
const VIDEO = manifest.videos.SynctexIpcExtensions;

const Scene1AutoReload: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const reloadedAt = Math.round(durationInFrames * 0.5);
  const reloaded = frame >= reloadedAt;

  return (
    <div style={{ display: 'flex', gap: 18, alignItems: 'stretch', width: 1860, height: 850 }}>
      {/* Terminal pane */}
      <div
        style={{
          width: 600,
          borderRadius: 16,
          backgroundColor: '#020617',
          border: '1.5px solid rgba(34, 211, 238, 0.45)',
          padding: 24,
          fontFamily: FONT_MONO,
          fontSize: 18.5,
          lineHeight: 1.58,
          color: '#e2e8f0',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <div style={{ fontSize: 16, fontWeight: 800, color: '#22d3ee', marginBottom: 12 }}>
            TERMINAL • TeX / Typst Watcher
          </div>
          <div style={{ color: '#94a3b8' }}>$ latexmk -pdf -synctex=1 -pvc paper.tex</div>
          <div style={{ marginTop: 14 }}>
            <div>[16:40:02] Writing paper.pdf (12 pp)</div>
            <div>[16:40:02] Writing paper.synctex.gz</div>
            {reloaded && (
              <>
                <div style={{ color: '#34d399', marginTop: 12, fontWeight: 700 }}>
                  [16:40:03] mtime changed (800ms)
                </div>
                <div style={{ color: '#67e8f9', fontWeight: 700 }}>
                  [16:40:03] Reloaded Rev 2.0 • Anchored
                </div>
              </>
            )}
          </div>
        </div>
        <div
          style={{
            padding: 14,
            borderRadius: 11,
            backgroundColor: 'rgba(15, 23, 42, 0.92)',
            border: '1px solid #334155',
            fontFamily: FONT_SANS,
            fontSize: 18,
            color: '#cbd5e1',
          }}
        >
          Zero flicker — scroll anchor, zoom, and split panes remain locked.
        </div>
      </div>

      {/* Pinleaf Window */}
      <PinleafWindow
        widthPx={1242}
        heightPx={850}
        autoReloadEnabled
        keyPressChip={reloaded ? '⟳ Reloaded Rev 2.0 (800 ms)' : 'Watching paper.pdf on disk…'}
        trailNodes={[
          { id: 1, frozen: true },
          { id: 2, active: true },
        ]}
        panes={[
          {
            label: 'Pane #1 • Locked',
            frozen: true,
            pageProps: { pageNumber: 1, focusSection: 'intro', autoReloadVersion: reloaded ? 2 : 1 },
          },
          {
            label: 'Pane #2 • Live Manuscript',
            active: true,
            pageProps: {
              pageNumber: 3,
              focusSection: 'theorem',
              autoReloadVersion: reloaded ? 2 : 1,
            },
          },
        ]}
        statusBanner={
          reloaded
            ? { text: 'Reloaded Rev 2.0 (Anchor preserved)', tone: 'ok' }
            : undefined
        }
      />
    </div>
  );
};

const Scene2TwoWaySyncTex: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const forwardAt = Math.round(durationInFrames * 0.25);
  const inverseAt = Math.round(durationInFrames * 0.62);
  const forwardActive = frame >= forwardAt;
  const inverseActive = frame >= inverseAt;

  return (
    <div style={{ display: 'flex', gap: 18, alignItems: 'stretch', width: 1860, height: 850 }}>
      {/* Editor / TeX side */}
      <div
        style={{
          width: 620,
          borderRadius: 16,
          backgroundColor: '#020617',
          border: '1.5px solid rgba(34, 211, 238, 0.45)',
          padding: 24,
          fontFamily: FONT_MONO,
          fontSize: 18,
          lineHeight: 1.62,
          color: '#cbd5e1',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <div style={{ fontSize: 16, fontWeight: 800, color: '#22d3ee', marginBottom: 12 }}>
            EDITOR • main.tex (.synctex.gz)
          </div>
          {[
            '39  \\section{Incremental}',
            '40  ',
            '41  \\begin{theorem}[Sync]',
            '42    Wait-free page tiles',
            '43    update SyncTeX bounds.',
            '44  \\end{theorem}',
          ].map((line, i) => {
            const isLine42 = line.startsWith('42');
            const lit = isLine42 && (forwardActive || inverseActive);
            return (
              <div
                key={i}
                style={{
                  padding: '3px 10px',
                  borderRadius: 7,
                  backgroundColor: lit
                    ? inverseActive
                      ? 'rgba(52, 211, 153, 0.28)'
                      : 'rgba(34, 211, 238, 0.22)'
                    : 'transparent',
                  borderLeft: lit ? '4px solid #22d3ee' : '4px solid transparent',
                  color: lit ? '#f8fafc' : '#94a3b8',
                  fontWeight: lit ? 700 : 500,
                }}
              >
                {line}
              </div>
            );
          })}

          <div
            style={{
              marginTop: 20,
              padding: 14,
              borderRadius: 11,
              backgroundColor: 'rgba(15, 23, 42, 0.94)',
              border: '1.5px solid #334155',
              fontSize: 16.5,
              color: '#67e8f9',
            }}
          >
            <div>$ pinleaf --synctex-forward 42:1:main.tex paper.pdf</div>
            {inverseActive && (
              <div style={{ color: '#34d399', marginTop: 8, fontWeight: 700 }}>
                ⇧⌘J / ⌘+Click → code -g main.tex:42:5
              </div>
            )}
          </div>
        </div>

        <div style={{ fontFamily: FONT_SANS, fontSize: 18, color: '#cbd5e1' }}>
          Zero C FFI: pure-Rust SyncTeX v1 parser decodes gzipped node records directly.
        </div>
      </div>

      {/* Pinleaf side */}
      <PinleafWindow
        widthPx={1222}
        heightPx={850}
        keyPressChip={
          inverseActive
            ? '⌘+Click / ⇧⌘J  →  main.tex:42'
            : forwardActive
            ? 'Forward Search  →  Line 42 Highlighted'
            : 'Ready for SyncTeX Lookup'
        }
        panes={[
          {
            label: 'Pane #1 • SyncTeX Target (Page 3)',
            active: true,
            pageProps: {
              pageNumber: 3,
              focusSection: 'theorem',
              synctexHighlightTopPx: forwardActive ? 316 : null,
            },
          },
        ]}
        statusBanner={
          inverseActive
            ? { text: 'SyncTeX Inverse -> main.tex:42 (editor opened)', tone: 'ok' }
            : undefined
        }
      />
    </div>
  );
};

const Scene3CliIpc: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const cmd2At = Math.round(durationInFrames * 0.38);
  const cmd3At = Math.round(durationInFrames * 0.68);
  const step = frame < cmd2At ? 1 : frame < cmd3At ? 2 : 3;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, alignItems: 'center' }}>
      {/* CLI Banner */}
      <div
        style={{
          width: 1860,
          borderRadius: 14,
          backgroundColor: '#020617',
          border: '1.5px solid rgba(34, 211, 238, 0.5)',
          padding: '13px 22px',
          fontFamily: FONT_MONO,
          fontSize: 20,
          fontWeight: 700,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          color: '#e2e8f0',
        }}
      >
        <div style={{ display: 'flex', gap: 16 }}>
          <span style={{ color: '#22d3ee', fontWeight: 800 }}>$</span>
          <span>
            {step === 1
              ? 'pinleaf --execute-command goto_page 3'
              : step === 2
              ? 'pinleaf --execute-command split_right'
              : 'pinleaf --execute-command cycle_color_mode'}
          </span>
        </div>
        <div style={{ display: 'flex', gap: 16, fontSize: 18 }}>
          <span style={{ color: '#34d399' }}>→ queued (exit 0)</span>
          <span style={{ color: '#94a3b8' }}>$TMPDIR/pinleaf-$USER.sock (0600)</span>
        </div>
      </div>

      <PinleafWindow
        heightPx={772}
        theme={step === 3 ? 'sepia' : 'light'}
        keyPressChip={`IPC Command ${step}/3 Executed Live`}
        panes={
          step >= 2
            ? [
                {
                  label: 'Pane #1 • Page 3',
                  frozen: true,
                  pageProps: { pageNumber: 3, focusSection: 'theorem' },
                },
                {
                  label: 'Pane #2 • IPC Split Right',
                  active: true,
                  pageProps: { pageNumber: 4, focusSection: 'figure' },
                },
              ]
            : [
                {
                  label: 'Pane #1 • Page 3 (via IPC goto_page 3)',
                  active: true,
                  pageProps: { pageNumber: 3, focusSection: 'theorem' },
                },
              ]
        }
      />
    </div>
  );
};

const Scene4ShellExtensions: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const ranAt = Math.round(durationInFrames * 0.56);
  const ran = frame >= ranAt;

  return (
    <PinleafWindow
      keyPressChip={ran ? 'Executed Extension [Ctrl+Y]' : 'Pressed  ⇧⌘E  →  Shell & Python Extensions'}
      panes={[
        {
          label: 'Pane #1 • Selected Quote Ready for External Tool',
          active: true,
          pageProps: { pageNumber: 1, focusSection: 'intro', activeSelectionText: true },
        },
      ]}
      libraryModal={{
        heading: 'Extensions & External Commands (⇧⌘E)',
        rows: [
          {
            title: 'Copy Formatted Citation & Quote',
            meta: `printf '%s (p.%s)' {selected_text} {page} | pbcopy  •  PINLEAF_DOI=10.1145/3597503.3623310`,
            badge: 'Ctrl+Y',
            selected: true,
          },
          {
            title: 'Send Abstract to Zotero / Notes Script',
            meta: 'python3 ~/scripts/send_to_notes.py %{file_path} %{page_number} %{doi}',
            badge: 'Alt+Z',
          },
          {
            title: 'Ask Local LLM about Selection',
            meta: 'ollama run llama3.2 "Summarize theorem:" {selected_text}',
            badge: 'Palette',
          },
        ],
      }}
      statusBanner={
        ran
          ? {
              text: 'Shell-safe single-quoting • Copied formatted quote + DOI',
              tone: 'ok',
            }
          : undefined
      }
    />
  );
};

export const Video5SynctexIpc: React.FC = () => {
  return (
    <VideoSequenceShell
      video={VIDEO}
      transitionFrames={manifest.transitionFrames}
      renderScene={(idx) => {
        switch (idx) {
          case 0:
            return <Scene1AutoReload />;
          case 1:
            return <Scene2TwoWaySyncTex />;
          case 2:
            return <Scene3CliIpc />;
          case 3:
          default:
            return <Scene4ShellExtensions />;
        }
      }}
    />
  );
};
