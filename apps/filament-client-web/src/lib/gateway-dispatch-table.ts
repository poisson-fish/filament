export type DecodedGatewayEvent = {
  type: string;
  payload: unknown;
};

export type GatewayDispatchTable<
  TEvent extends DecodedGatewayEvent,
  THandlers,
> = {
  [K in TEvent["type"]]: (
    payload: Extract<TEvent, { type: K }>["payload"],
    handlers: THandlers,
  ) => void;
};

export function dispatchDecodedGatewayEvent<
  TEvent extends DecodedGatewayEvent,
  THandlers,
>(
  event: TEvent,
  handlers: THandlers,
  dispatchTable: GatewayDispatchTable<TEvent, THandlers>,
): void {
  const dispatch = dispatchTable[event.type as TEvent["type"]] as (
    payload: TEvent["payload"],
    targetHandlers: THandlers,
  ) => void;
  dispatch(event.payload, handlers);
}
