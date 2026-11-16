import React from 'react';
import { interpolate, useCurrentFrame, useVideoConfig } from 'remotion';
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
    <div style={{ display: 'flex', gap: 18, alignItems: 'stretch', width: 1520, height: 760 }}>
      {/* Terminal pane */}
      <div
        style={{
          width: 470,
          borderRadius: 16,
          backgroundColor: '#020617',
          border: '1px solid rgba(34, 211, 238, 0.38)',
          padding: 20,
          fontFamily: FONT_MONO,
          fontSize: 13.5,
          lineHeight: 1.62,
          color: '#e2e8f0',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <div style={{ fontSize: 12, fontWeight: 700, color: '#22d3ee', marginBottom: 10 }}>
            TERMINAL • TeX / Typst Watcher
          </div>
          <div style={{ color: '#94a3b8' }}>$ latexmk -pdf -synctex=1 -pvc paper.tex</div>
          <div style={{ marginTop: 10 }}>
            <div>[16:40:02] Writing paper.pdf (12 pages)…</div>
            <div>[16:40:02] Writing paper.synctex.gz…</div>
            {reloaded && (
              <>
                <div style={{ color: '#34d399', marginTop: 8 }}>
                  [16:40:03] Detected mtime change in 800ms
                </div>
                <div style={{ color: '#67e8f9' }}>
                  [16:40:03] Reloaded Rev 2.0 • Scroll preserved
                </div>
              </>
            )}
          </div>
        </div>
        <div
          style={{
            padding: 12,
            borderRadius: 10,
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            border: '1px solid #334155',
            fontSize: 12.5,
            color: '#cbd5e1',
          }}
        >
          No flicker, no lost scroll position, split panes remain pinned.
        </div>
      </div>

      {/* Pinleaf Window */}
      <PinleafWindow
        widthPx={1030}
        heightPx={760}
        autoReloadEnabled
        keyPressChip={reloaded ? '⟳ Reloaded Rev 2.0 (800 ms)' : 'Watching paper.pdf on disk…'}
        trailNodes={[
          { id: 1, frozen: true },
          { id: 2, active: true },
        ]}
        panes={[
          {
            label: 'Pane #1 • Frozen Anchor',
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
            ? { text: 'Live Auto-Reload complete: Rev 2.0 (Anchor & zoom preserved)', tone: 'ok' }
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
    <div style={{ display: 'flex', gap: 18, alignItems: 'stretch', width: 1520, height: 760 }}>
      {/* Editor / TeX side */}
      <div
        style={{
          width: 500,
          borderRadius: 16,
          backgroundColor: '#020617',
          border: '1.5px solid rgba(34, 211, 238, 0.4)',
          padding: 20,
          fontFamily: FONT_MONO,
          fontSize: 13.5,
          lineHeight: 1.65,
          color: '#cbd5e1',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <div style={{ fontSize: 12, fontWeight: 700, color: '#22d3ee', marginBottom: 10 }}>
            EDITOR • main.tex (.synctex.gz paired)
          </div>
          {[
            '39  \\section{Incremental Proofs}',
            '40  ',
            '41  \\begin{theorem}[Incremental Sync]',
            '42    Wait-free rasterization updates',
            '43    SyncTeX bounds in O(1) lookup.',
            '44  \\end{theorem}',
          ].map((line, i) => {
            const isLine42 = line.startsWith('42');
            const lit = isLine42 && (forwardActive || inverseActive);
            return (
              <div
                key={i}
                style={{
                  padding: '2px 8px',
                  borderRadius: 6,
                  backgroundColor: lit
                    ? inverseActive
                      ? 'rgba(52, 211, 153, 0.25)'
                      : 'rgba(34, 211, 238, 0.2)'
                    : 'transparent',
                  borderLeft: lit ? '3px solid #22d3ee' : '3px solid transparent',
                  color: lit ? '#f8fafc' : '#94a3b8',
                }}
              >
                {line}
              </div>
            );
          })}

          <div
            style={{
              marginTop: 18,
              padding: 12,
              borderRadius: 10,
              backgroundColor: 'rgba(15, 23, 42, 0.92)',
              border: '1px solid #334155',
              fontSize: 12,
              color: '#67e8f9',
            }}
          >
            <div>$ pinleaf --synctex-forward 42:1:main.tex paper.pdf</div>
            {inverseActive && (
              <div style={{ color: '#34d399', marginTop: 6 }}>
                ⇧⌘J / ⌘+Click → code -g main.tex:42:5
              </div>
            )}
          </div>
        </div>

        <div style={{ fontFamily: FONT_SANS, fontSize: 13, color: '#94a3b8' }}>
          Zero C FFI: pure-Rust SyncTeX v1 parser decodes gzipped node records directly.
        </div>
      </div>

      {/* Pinleaf side */}
      <PinleafWindow
        widthPx={1000}
        heightPx={760}
        keyPressChip={
          inverseActive
            ? '⌘+Click / ⇧⌘J  →  Inverse Jump main.tex:42'
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
              synctexHighlightTopPx: forwardActive ? 290 : null,
            },
          },
        ]}
        statusBanner={
          inverseActive
            ? { text: 'SyncTeX Inverse -> main.tex:42 (editor opened cleanly)', tone: 'ok' }
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
          width: 1520,
          borderRadius: 14,
          backgroundColor: '#020617',
          border: '1px solid rgba(34, 211, 238, 0.45)',
          padding: '12px 20px',
          fontFamily: FONT_MONO,
          fontSize: 14,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          color: '#e2e8f0',
        }}
      >
        <div style={{ display: 'flex', gap: 16 }}>
          <span style={{ color: '#22d3ee', fontWeight: 700 }}>$</span>
          <span>
            {step === 1
              ? 'pinleaf --execute-command goto_page 3'
              : step === 2
              ? 'pinleaf --execute-command split_right'
              : 'pinleaf --execute-command cycle_color_mode'}
          </span>
        </div>
        <div style={{ display: 'flex', gap: 12, fontSize: 12.5 }}>
          <span style={{ color: '#34d399' }}>→ queued (exit 0)</span>
          <span style={{ color: '#64748b' }}>Socket: $TMPDIR/pinleaf-$USER.sock (0600)</span>
        </div>
      </div>

      <PinleafWindow
        heightPx={675}
        theme={step === 3 ? 'sepia' : 'light'}
        keyPressChip={`IPC Step ${step}/3 Executed Live`}
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
            meta: `printf '%s (p.%s)' {selected_text} {page} | pbcopy   •   PINLEAF_DOI=10.1145/3597503.3623310`,
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
              text: 'Shell-safe single-quoting prevents injection • Copied formatted quote + DOI to clipboard',
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
