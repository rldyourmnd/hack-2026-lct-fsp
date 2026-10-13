import { openobserveRum } from '@openobserve/browser-rum';
import { canReplay, safeUrl, sanitizeEvent } from './privacy';

let initialized = false;

export function startObservability() {
  if (!initialized) {
    openobserveRum.init({
      applicationId: 'fsp-career',
      clientToken: 'same-origin-ingestion',
      site: window.location.host,
      organizationIdentifier: 'default',
      service: 'fsp-web',
      env: 'demo',
      version: import.meta.env.VITE_RELEASE_SHA ?? 'development',
      proxy: ({ path }) => (path.endsWith('/replay') ? '/observe/replay' : '/observe/rum'),
      sessionSampleRate: 100,
      sessionReplaySampleRate: 100,
      defaultPrivacyLevel: 'mask',
      enablePrivacyForActionName: true,
      startSessionReplayRecordingManually: true,
      trackViewsManually: true,
      trackResources: true,
      trackLongTasks: true,
      trackUserInteractions: true,
      compressIntakeRequests: false,
      telemetrySampleRate: 0,
      telemetryConfigurationSampleRate: 0,
      telemetryUsageSampleRate: 0,
      sessionPersistence: 'memory',
      beforeSend: sanitizeEvent,
      allowedTracingUrls: [{ match: `${window.location.origin}/api/v1/`, propagatorTypes: ['tracecontext'] }],
      propagateTraceBaggage: false,
    });
    initialized = true;
  }
  return {
    stopReplay: () => openobserveRum.stopSessionReplayRecording(),
    view: () => {
      openobserveRum.stopSessionReplayRecording();
      openobserveRum.startView(new URL(safeUrl(window.location.href)).pathname);
      if (canReplay(window.location.href)) openobserveRum.startSessionReplayRecording();
    },
  };
}
