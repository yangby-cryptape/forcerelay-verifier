from http.server import BaseHTTPRequestHandler, HTTPServer
import re
import os.path

count = 0
dir = os.path.dirname(__file__)

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        headers_match_obj = re.match(r'/eth/v1/beacon/headers/([\d]+)', self.path)
        if headers_match_obj:
            file_path = dir + '/mock_data/headers/{}.json'.format(headers_match_obj.group(1))
            if os.path.exists(file_path):
                self.send_response(200)
                self.send_header('Content-type', 'application/json')
                self.end_headers()
                content = open(file_path, 'rb').read()
                self.wfile.write(content)
            else:
                self.send_response(404)
        blocks_match_obj = re.match(r'/eth/v2/beacon/blocks/([\d]+)', self.path)
        if blocks_match_obj:
            file_path = dir + '/mock_data/blocks/{}.json'.format(blocks_match_obj.group(1))
            if os.path.exists(file_path):
                self.send_response(200)
                self.send_header('Content-type', 'application/json')
                self.end_headers()
                content = open(file_path, 'rb').read()
                self.wfile.write(content)
            else:
                self.send_response(404)
        elif self.path == '/eth/v1/beacon/light_client/bootstrap/0xe06056afdb9a0a9fd7fbaf89bb0e96eced24de0104bc5b7e3960c115d6990f90':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            content = open(dir + '/mock_data/bootstrap.json', 'rb').read()
            self.wfile.write(content)
        elif self.path == '/eth/v1/beacon/light_client/updates?start_period=706&count=128':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            content = open(dir + '/mock_data/updates.json', 'rb').read()
            self.wfile.write(content)
        elif self.path == '/eth/v1/beacon/light_client/updates?start_period=706&count=1':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            content = open(dir + '/mock_data/updates.json', 'rb').read()
            self.wfile.write(content)
        elif self.path == '/eth/v1/beacon/light_client/finality_update' or self.path == '/eth/v1/beacon/light_client/optimistic_update':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            global count
            count += 1
            if count <= 5:
                content = open(dir + '/mock_data/finality_update1.json', 'rb').read()
                self.wfile.write(content)
            else:
                content = open(dir + '/mock_data/finality_update2.json', 'rb').read()
                self.wfile.write(content)
        else:
            print(self.path)
            self.send_response(404)

    def do_POST(self):
        print(self.path)
        content_len = int(self.headers.get('Content-Length'))
        post_body = self.rfile.read(content_len)
        if post_body == b'{"id":1,"jsonrpc":"2.0","method":"eth_getTransactionByHash","params":["0x40c6bbef3f8cb9681e0bafc42d20ec421c706633324496b7434ac719e824e027"]}':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            content = open(dir + '/mock_data/transaction.json', 'rb').read()
            self.wfile.write(content)
        elif post_body == b'{"id":2,"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0x40c6bbef3f8cb9681e0bafc42d20ec421c706633324496b7434ac719e824e027"]}':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            content = open(dir + '/mock_data/transaction_receipt.json', 'rb').read()
            self.wfile.write(content)
        elif post_body == b'{"id":2,"jsonrpc":"2.0","method":"eth_getBlockReceipts","params":["0xfd94e0"]}':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            content = open(dir + '/mock_data/block_receipt.json', 'rb').read()
            self.wfile.write(content)
        else:
            print(post_body)
            self.send_response(404)
        pass


if __name__ == '__main__':
    server = HTTPServer(('', 8444), Handler)
    print('start the server')
    server.serve_forever()

