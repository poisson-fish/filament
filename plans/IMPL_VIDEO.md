# Implement Video Stream View

Design a view screen for video/webcam streams mimicking Discord's layout, where the main channel view splits to show a stream grid and the text chat moves to the side.

## Design Philosophy
We must ensure the feature is fully componentized. We will avoid adding complex state (like stream participants, toggling logic) directly into `AppShellLayout` or `AppShellPage`. Instead, we will create self-contained container components that pull the state they need from the application runtime/context.

## Proposed Changes

### UI & Layout

#### [NEW] StreamColumn.tsx(file:///Users/twin/Documents/filament/apps/filament-client-web/src/features/app-shell/components/stream/StreamColumn.tsx)
- This is the main container component for the video stream feature.
- It will internally hook into the `AppShellRuntime` (or relevant contexts) to get the `rtcSnapshot`, `canToggleVoiceCamera`, `canToggleVoiceScreenShare`, etc.
- It will conditionally render the `StreamGrid` and `StreamControls` based on the internal state.
- It maps over the `rtcSnapshot.videoTracks` and renders a `VideoTile` for each.
- Handles responsive CSS grid logic for arranging streams into 1, 2, 3, or quadrants.

#### [NEW] VideoTile.tsx(file:///Users/twin/Documents/filament/apps/filament-client-web/src/features/app-shell/components/stream/VideoTile.tsx)
- Takes a `RtcVideoTrackSnapshot` stream source as a prop.
- Uses standard SolidJS refs to bind the video track dynamically via the browser's `HTMLVideoElement`.
- Handles placeholder/fallback UI when video is establishing or off.

#### [NEW] StreamControls.tsx(file:///Users/twin/Documents/filament/apps/filament-client-web/src/features/app-shell/components/stream/StreamControls.tsx)
- Replicates the bottom control row featured in the second screenshot.
- Triggers standard `toggleVoiceCamera`, `toggleVoiceScreenShare`, `toggleVoiceMicrophone`, `toggleVoiceDeafen` and voice leave functions from the backend controllers. These actions will be passed as props or picked up from context via the parent `StreamColumn`.

#### [MODIFY] AppShellLayout.tsx(file:///Users/twin/Documents/filament/apps/filament-client-web/src/features/app-shell/components/layout/AppShellLayout.tsx)
- Add a new `streamColumn?: JSX.Element` prop.
- Render `streamColumn` conditionally before `chatColumn` when provided.
- Update CSS context/classes in `app-shell-scaffold` to gracefully split the screen (e.g., center takes priority, text chat shifts right) when `streamColumn` is present.

#### [MODIFY] AppShellPage.tsx(file:///Users/twin/Documents/filament/apps/filament-client-web/src/pages/AppShellPage.tsx)
- Check `voiceState.rtcSnapshot()` or `isVoiceSessionActive()` to determine if the user is in an active voice session where they might want to view the grid.
- Pass the self-contained `<StreamColumn />` component to `AppShellLayout` when appropriate (e.g. `isVoiceSessionActive === true`). We only pass the component, and `StreamColumn` handles its own internal data fetching for streams.

## Verification Plan

### Automated Tests
- Verify the integrity of the build and any existing UI layout tests:
  `cd apps/filament-client-web && npm run test` or standard Vitest suite.

### Manual Verification
1. Start `filament-server` and `filament-client-web`.
2. Open the application in the browser and join a Voice Channel.
3. Upon joining, verify that the central space divides to provide a `StreamGrid`, positioning text chat in the right rail.
4. Use the `StreamControls` at the bottom to "Enable Camera" or "Share Screen".
5. Observe the LiveKit `RtcVideoTrackSnapshot` successfully populating a quadrant on the screen for the active stream.
6. Verify layout responsively manages single streams vs. multiple participants sharing.
