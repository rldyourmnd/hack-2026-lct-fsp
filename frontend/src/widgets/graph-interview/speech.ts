export function speechErrorMessage(error: string) {
  if (error === 'not-allowed' || error === 'service-not-allowed')
    return 'Браузер не дал доступ к микрофону. Разрешите его для этого сайта и попробуйте снова.';
  if (error === 'no-speech') return 'Речь не была услышана. Проверьте микрофон и начните ответ ещё раз.';
  if (error === 'audio-capture') return 'Браузер не видит микрофон. Подключите его или выберите текстовый ответ.';
  if (error === 'network') return 'Сервис распознавания браузера недоступен. Можно повторить или ответить текстом.';
  return 'Распознавание остановилось. Можно продолжить голосом с того же места.';
}

export function submittedResponseMode(inputMode: 'voice' | 'text', voiceCaptured: boolean) {
  return inputMode === 'voice' && voiceCaptured ? ('voice' as const) : ('text' as const);
}
