//
// waSCC runtime for AWS Lambda example configuration.
//

terraform {
  required_version = ">= 0.12.19"
}


# Build from https://github.com/terraform-providers/terraform-provider-aws/commit/df71a4fd95c0e5a9afe5b08c43a951d3a7fda0ed.
# Will be released in v2.59.0.
# provider "aws" {
#   version = ">= 2.58.0"
# }

//
// Data sources for current AWS account ID, partition and region.
//

data "aws_caller_identity" "current" {}

data "aws_partition" "current" {}

data "aws_region" "current" {}

//
// API Gateway resources.
//

resource "aws_apigatewayv2_api" "example" {
  name          = "waSCC-example-apigw"
  protocol_type = "HTTP"
  target        = aws_lambda_function.example.arn
}

//
// Lambda resources.
//

data "aws_lambda_layer_version" "slim" {
  layer_name = "wascc-slim"
}

resource "aws_lambda_function" "example" {
  filename         = "${path.module}/app.zip"
  source_code_hash = filebase64sha256("${path.module}/app.zip")
  function_name    = "waSCC-example-apigw"
  role             = aws_iam_role.example.arn
  handler          = "doesnt.matter"
  runtime          = "provided"
  memory_size      = 256
  timeout          = 90

  layers = [data.aws_lambda_layer_version.slim.arn]

  environment {
    variables = {
      RUST_BACKTRACE = "1"
      RUST_LOG       = "info,cranelift_wasm=warn,cranelift_codegen=info"
    }
  }
}

// See https://docs.aws.amazon.com/lambda/latest/dg/services-apigateway.html#apigateway-permissions.
resource "aws_lambda_permission" "example" {
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.example.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "arn:${data.aws_partition.current.partition}:execute-api:${data.aws_region.current.name}:${data.aws_caller_identity.current.account_id}:${aws_apigatewayv2_api.example.id}/*/$default"
}

//
// IAM resources.
//

resource "aws_iam_role" "example" {
  name = "waSCC-example-apigw-Lambda-role"

  assume_role_policy = <<EOT
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": {
      "Service": "lambda.amazonaws.com"
    },
    "Action": "sts:AssumeRole"
  }]
}
EOT
}

resource "aws_iam_policy" "cloudwatch_logs_policy" {
  name = "waSCC-example-apigw-Lambda-CloudWatchLogsPolicy"

  policy = <<EOT
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "logs:CreateLogGroup",
      "Resource": "arn:${data.aws_partition.current.partition}:logs:${data.aws_region.current.name}:${data.aws_caller_identity.current.account_id}:*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "logs:CreateLogStream",
        "logs:PutLogEvents"
      ],
      "Resource": [
        "arn:${data.aws_partition.current.partition}:logs:${data.aws_region.current.name}:${data.aws_caller_identity.current.account_id}:log-group:/aws/lambda/${aws_lambda_function.example.function_name}:*"
      ]
    }
  ]
}
EOT
}

resource "aws_iam_role_policy_attachment" "cloudwatch_logs" {
  role       = aws_iam_role.example.name
  policy_arn = aws_iam_policy.cloudwatch_logs_policy.arn
}

//
// Outputs.
//

output "FunctionName" {
  value = aws_lambda_function.example.function_name
}

output "Url" {
  value = aws_apigatewayv2_api.example.api_endpoint
}
