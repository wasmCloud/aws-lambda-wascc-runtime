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

//
// Data sources for current AWS account ID, partition and region.
//

data "aws_caller_identity" "current" {}

data "aws_partition" "current" {}

data "aws_region" "current" {}

//
// Lambda resources.
//

resource "aws_lambda_function" "example" {
  filename         = "${path.module}/app.zip"
  source_code_hash = filebase64sha256("${path.module}/app.zip")
  function_name    = "waSCC-example-sqs"
  role             = aws_iam_role.example.arn
  handler          = "doesnt.matter"
  runtime          = "provided"
  memory_size      = 256
  timeout          = 90

  environment {
    variables = {
      RUST_BACKTRACE = "1"
      RUST_LOG       = "info,cranelift_wasm=warn",
    }
  }
}

//
// IAM resources.
//

resource "aws_iam_role" "example" {
  name = "waSCC-example-sqs-Lambda-role"

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
  name = "waSCC-example-sqs-Lambda-CloudWatchLogsPolicy"

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

resource "aws_iam_policy" "sqs_policy" {
  name = "waSCC-example-sqs-Lambda-SQSPolicy"

  policy = <<EOT
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "sqs:ReceiveMessage",
        "sqs:DeleteMessage",
        "sqs:GetQueueAttributes"
      ],
      "Resource": [
        "${aws_sqs_queue.request.arn}"
      ]
    },
    {
      "Effect": "Allow",
      "Action": [
        "sqs:SendMessage"
      ],
      "Resource": [
        "${aws_sqs_queue.reply.arn}"
      ]
    }
  ]
}
EOT
}

resource "aws_iam_role_policy_attachment" "sqs" {
  role       = aws_iam_role.example.name
  policy_arn = aws_iam_policy.sqs_policy.arn
}

resource "aws_sqs_queue" "request" {
  name                       = "waSCC-example-sqs-request"
  visibility_timeout_seconds = 600
}

resource "aws_lambda_event_source_mapping" "example" {
  event_source_arn = aws_sqs_queue.request.arn
  function_name    = aws_lambda_function.example.arn
}

resource "aws_sqs_queue" "reply" {
  name                       = "waSCC-example-sqs-reply"
  visibility_timeout_seconds = 60
}

//
// Outputs.
//

output "RequestQueueUrl" {
  value = aws_sqs_queue.request.id
}

output "ReplyQueueUrl" {
  value = aws_sqs_queue.reply.id
}
