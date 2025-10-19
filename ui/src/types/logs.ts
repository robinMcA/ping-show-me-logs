export interface Root {
  result: Result[];
  pagedResultCooke: any;
  totalPagedResultsPolicy: string;
  totalPagedResults: number;
  remainingPagedResults: number;
}

export interface Result {
  payload: Payload;
  timestamp: string;
  type: string;
  source: string;
}

export interface Payload {
  context?: string;
  level: string;
  logger?: string;
  mdc?: Mdc;
  message?: string;
  thread?: string;
  timestamp: string;
  transactionId: string;
  _id?: string;
  client?: Client;
  component?: string;
  eventName?: string;
  http?: Http;
  realm?: string;
  source?: string;
  topic?: string;
}

export interface Mdc {
  transactionId: string;
}

export interface Client {
  ip: string;
}

export interface Http {
  request: Request;
}

export interface Request {
  headers: Headers;
  method: string;
  path: string;
  queryParameters: QueryParameters;
  secure: boolean;
}

export interface Headers {
  accept: string[];
  "accept-api-version": string[];
  "content-type": string[];
  host: string[];
  origin: string[];
  "user-agent": string[];
  "x-forwarded-for": string[];
  "x-forwarded-proto": string[];
  "x-real-ip": string[];
  "x-requested-with": string[];
}

export interface QueryParameters {
  authIndexType: string[];
  authIndexValue: string[];
}
