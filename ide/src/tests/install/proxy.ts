import * as mockttp from "mockttp";
import path from "path";

export const PROXY_PORT = 8000;

export const createProxy = async (zip: string) => {
    // Create a proxy server with a self-signed HTTPS CA certificate:
    const https = await mockttp.generateCACertificate();
    const server = mockttp.getLocal({ https });

    // Intercept only requests made for zips on Github
    server.anyRequest().thenPassThrough();
    server.get(/github.com.*.zip/).thenFromFile(200, path.resolve(zip))

    await server.start(PROXY_PORT);
    return server;
};
