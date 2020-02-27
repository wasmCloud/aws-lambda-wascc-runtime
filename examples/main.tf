//
// waSCC runtime for AWS Lambda example configuration.
//

terraform {
  required_version = ">= 0.12.19"
}

provider "aws" {
  version = ">= 2.50.0"

  region = "us-west-2"
}

provider "archive" {
  version = ">= 1.3.0"
}

//
// Data sources for current AWS account ID, partition and region.
//

data "aws_caller_identity" "current" {}

data "aws_partition" "current" {}

data "aws_region" "current" {}

//
// Lambda resources.
//

data "archive_file" "lambda_zip" {
  type        = "zip"
  source_file = "bootstrap"
  output_path = "${path.module}/app.zip"

  # source {
  #   content  = ""
  #   filename = "bootstrap"
  # }
}

resource "aws_lambda_function" "example" {
  filename         = data.archive_file.lambda_zip.output_path
  source_code_hash = data.archive_file.lambda_zip.output_base64sha256
  function_name    = "waSCC-example"
  role             = aws_iam_role.example.arn
  handler          = "doesnt.matter"
  runtime          = "provided"
  memory_size      = 256

  environment {
    variables = {
      RUST_BACKTRACE = "1"
      RUST_LOG       = "debug"
    }
  }
}

//
// IAM resources.
//

resource "aws_iam_role" "example" {
  name = "waSCC-example-Lambda-role"

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
  name = "CellCloudWatchLogsPolicy"

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
