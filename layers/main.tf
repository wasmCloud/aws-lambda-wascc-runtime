//
// Layers for the waSCC runtime for AWS Lambda.
//

terraform {
  required_version = ">= 0.12.19"
}

provider "aws" {
  version = ">= 3.3.0"
}

//
// Layers.
//

resource "aws_lambda_layer_version" "slim" {
  layer_name  = "wascc-slim-al2"
  description = "waSCC custom runtime (slim)"

  filename         = "${path.module}/slim.zip"
  source_code_hash = filebase64sha256("${path.module}/slim.zip")

  compatible_runtimes = ["provided.al2"]
}

//
// Outputs.
//

output "ARN" {
  value = aws_lambda_layer_version.slim.arn
}
